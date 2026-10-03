//! Expired retry helpers are removed. Production retry decisions stay on typed facts.
//! The source guard and route policy regressions remain; 0.6 compatibility fixtures do not.

mod source_guard {
    use std::fs;
    use std::path::{Path, PathBuf};
    use syn::ext::IdentExt;
    use syn::visit::Visit;

    // ErrorCode::is_retryable is the canonical presentation classification, not a removed helper.
    const ALLOWED_FILES: &[&str] = &["src/utils/error/canonical.rs"];

    fn has_test_cfg(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            let syn::Meta::List(meta) = &attr.meta else {
                return false;
            };
            let cfg = meta.tokens.to_string().replace(' ', "");
            attr.path().is_ident("cfg")
                && (cfg == "test"
                    || cfg
                        .strip_prefix("all(")
                        .and_then(|cfg| cfg.strip_suffix(')'))
                        .is_some_and(|cfg| cfg.split(',').any(|term| term == "test")))
        })
    }

    fn item_attrs(item: &syn::Item) -> Option<&[syn::Attribute]> {
        match item {
            syn::Item::Const(item) => Some(&item.attrs),
            syn::Item::Enum(item) => Some(&item.attrs),
            syn::Item::ExternCrate(item) => Some(&item.attrs),
            syn::Item::Fn(item) => Some(&item.attrs),
            syn::Item::ForeignMod(item) => Some(&item.attrs),
            syn::Item::Impl(item) => Some(&item.attrs),
            syn::Item::Macro(item) => Some(&item.attrs),
            syn::Item::Mod(item) => Some(&item.attrs),
            syn::Item::Static(item) => Some(&item.attrs),
            syn::Item::Struct(item) => Some(&item.attrs),
            syn::Item::Trait(item) => Some(&item.attrs),
            syn::Item::TraitAlias(item) => Some(&item.attrs),
            syn::Item::Type(item) => Some(&item.attrs),
            syn::Item::Union(item) => Some(&item.attrs),
            syn::Item::Use(item) => Some(&item.attrs),
            _ => None,
        }
    }

    fn impl_item_attrs(item: &syn::ImplItem) -> Option<&[syn::Attribute]> {
        match item {
            syn::ImplItem::Const(item) => Some(&item.attrs),
            syn::ImplItem::Fn(item) => Some(&item.attrs),
            syn::ImplItem::Macro(item) => Some(&item.attrs),
            syn::ImplItem::Type(item) => Some(&item.attrs),
            _ => None,
        }
    }

    fn is_test_path(path: &str) -> bool {
        let file = path.rsplit('/').next().unwrap_or(path);
        let stem = file.strip_suffix(".rs").unwrap_or(file);
        stem == "tests" || stem == "provider_tests" || stem.ends_with("_tests")
    }

    struct Finder {
        file: String,
        context: String,
        violations: Vec<String>,
    }

    impl Finder {
        fn record(&mut self, what: &str) {
            self.violations
                .push(format!("{}: {what} in {}", self.file, self.context));
        }
    }

    impl<'ast> Visit<'ast> for Finder {
        fn visit_item(&mut self, item: &'ast syn::Item) {
            if item_attrs(item).is_some_and(has_test_cfg) {
                return;
            }
            syn::visit::visit_item(self, item);
        }

        fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
            if impl_item_attrs(item).is_some_and(has_test_cfg) {
                return;
            }
            syn::visit::visit_impl_item(self, item);
        }

        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            let old = std::mem::replace(&mut self.context, item.sig.ident.unraw().to_string());
            syn::visit::visit_item_fn(self, item);
            self.context = old;
        }

        fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
            let old = std::mem::replace(&mut self.context, item.sig.ident.unraw().to_string());
            syn::visit::visit_impl_item_fn(self, item);
            self.context = old;
        }

        fn visit_expr_method_call(&mut self, expr: &'ast syn::ExprMethodCall) {
            if expr.method.unraw() == "is_retryable" && expr.args.is_empty() {
                self.record(".is_retryable() call");
            }
            syn::visit::visit_expr_method_call(self, expr);
        }

        fn visit_expr_call(&mut self, expr: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = &*expr.func
                && let Some(last) = path.path.segments.last()
            {
                let name = last.ident.unraw().to_string();
                if name == "is_retryable_error" || name == "should_retry" {
                    self.record(&format!("{name}() call"));
                }
            }
            syn::visit::visit_expr_call(self, expr);
        }
    }

    fn collect_sources(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                // Test-only directories are not production routing/retry code.
                if matches!(name.as_deref(), Some("tests" | "provider_tests"))
                    || name.as_deref().is_some_and(|n| n.ends_with("_tests"))
                {
                    continue;
                }
                collect_sources(root, &path, out);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            out.push((rel, source));
        }
    }

    #[test]
    fn production_routing_retry_makes_no_deprecated_helper_calls() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut sources = Vec::new();
        collect_sources(root, &root.join("src"), &mut sources);
        assert!(
            sources.len() > 200,
            "production source inventory looks incomplete: {} files",
            sources.len()
        );

        let mut violations = Vec::new();
        for (rel, source) in &sources {
            if is_test_path(rel) || ALLOWED_FILES.contains(&rel.as_str()) {
                continue;
            }
            let file = syn::parse_file(source).unwrap_or_else(|e| panic!("{rel}: {e}"));
            let mut finder = Finder {
                file: rel.clone(),
                context: "<module>".to_string(),
                violations: Vec::new(),
            };
            finder.visit_file(&file);
            violations.extend(finder.violations);
        }

        assert!(
            violations.is_empty(),
            "deprecated retry helper call sites remain in production routing/retry code:\n{}",
            violations.join("\n")
        );
    }
}

// -------------------------------------------------------------------------------------
// 3. Focused RetryPolicy::decide tests — the batches/fine-tuning route retry decision.
// -------------------------------------------------------------------------------------

mod retry_policy_migration {
    use litellm_rs::core::providers::ProviderError;
    use litellm_rs::core::router::RouterConfig;
    use litellm_rs::core::router::retry_policy::{RetryContext, RetryPolicy};

    /// The exact decision expression the batches and fine-tuning routes now use for
    /// provider failover after D1E-c removed their `is_retryable_error` dependency.
    fn route_should_retry(error: &ProviderError) -> bool {
        RetryPolicy
            .decide(&RouterConfig::default(), error, RetryContext::unary(1, 2))
            .should_retry
    }

    #[test]
    fn transient_provider_failures_fail_over() {
        assert!(route_should_retry(&ProviderError::rate_limit("p", None)));
        assert!(route_should_retry(&ProviderError::network("p", "m")));
        assert!(route_should_retry(&ProviderError::timeout("p", "m")));
        assert!(route_should_retry(&ProviderError::provider_unavailable(
            "p", "m"
        )));
    }

    #[test]
    fn terminal_provider_failures_do_not_fail_over() {
        assert!(!route_should_retry(&ProviderError::authentication(
            "p", "m"
        )));
        assert!(!route_should_retry(&ProviderError::invalid_request(
            "p", "m"
        )));
        assert!(!route_should_retry(&ProviderError::model_not_found(
            "p", "m"
        )));
        assert!(!route_should_retry(&ProviderError::quota_exceeded(
            "p", "m"
        )));
    }

    #[test]
    fn typed_facts_upstream_status_drives_failover() {
        // Typed-facts policy: 5xx and 408 upstream statuses are retry candidates, which
        // is the behavior the routes inherit from RetryPolicy::decide after migration
        // (the deprecated is_retryable_error only retried Bedrock-modeled api errors).
        assert!(route_should_retry(&ProviderError::api_error(
            "p",
            503,
            "overloaded"
        )));
        assert!(route_should_retry(&ProviderError::api_error(
            "p", 408, "timeout"
        )));
        assert!(!route_should_retry(&ProviderError::api_error(
            "p", 404, "missing"
        )));
        assert!(!route_should_retry(&ProviderError::api_error(
            "p",
            400,
            "bad request"
        )));
    }
}
