# GitHub Models retirement audit

F10 / issue #1373, checked 2026-10-03. Changes are unreleased.

GitHub's [July 30 retirement announcement](https://github.blog/changelog/2026-07-30-github-models-is-now-retired/)
confirms that GitHub Models, including its catalog, inference API and BYOK, is no
longer available to any customer. The [July 1 notice](https://github.blog/changelog/2026-07-01-github-models-is-being-fully-retired-on-july-30-2026/)
announced that exact date; June's existing-customer exception no longer applies.
GitHub Copilot is a separate product and provider, retained by this change.

All 16 old static GitHub Models records lose callable metadata:

- gpt-4o, gpt-4o-mini, o1-preview, o1-mini
- meta-llama-3.1-405b-instruct, meta-llama-3.1-70b-instruct, meta-llama-3.1-8b-instruct
- mistral-large-2407, mistral-small-2409
- cohere-command-r-plus, cohere-command-r
- ai21-jamba-1.5-large, ai21-jamba-1.5-mini
- phi-3.5-moe-instruct, phi-3.5-mini-instruct, phi-3.5-vision-instruct

The canonical selector and its github-models alias retain their enum identity
but return the existing NotImplemented error before any transport. The callable
catalog and its invented zero-price static projection are removed. The central
historical price catalog is unchanged; prices do not restore callable status.
Tests exercise factory rejection and absent model projections, while existing
Copilot tests continue separately. No authenticated GitHub inference was called.

This batch depends on #1410's deletion of the unused native GitHub module. It
does not substitute a Foundry endpoint under the GitHub selector: credentials,
model IDs and protocol ownership differ and require explicit provider choice.
