use super::*;
use chrono::Utc;

// ==================== ReportTemplate Tests ====================

#[test]
fn test_report_template_creation() {
    let template = ReportTemplate {
        name: "Test Report".to_string(),
        description: "A test report template".to_string(),
        sections: vec![],
        format: ReportFormat::Pdf,
    };

    assert_eq!(template.name, "Test Report");
    assert_eq!(template.description, "A test report template");
    assert!(template.sections.is_empty());
}

#[test]
fn test_report_template_with_sections() {
    let template = ReportTemplate {
        name: "Full Report".to_string(),
        description: "Report with sections".to_string(),
        sections: vec![
            ReportSection {
                title: "Summary".to_string(),
                section_type: ReportSectionType::Summary,
                queries: vec!["stats".to_string()],
            },
            ReportSection {
                title: "Charts".to_string(),
                section_type: ReportSectionType::Chart,
                queries: vec!["chart_data".to_string()],
            },
        ],
        format: ReportFormat::Html,
    };

    assert_eq!(template.sections.len(), 2);
    assert_eq!(template.sections[0].title, "Summary");
    assert_eq!(template.sections[1].title, "Charts");
}

#[test]
fn test_report_template_clone() {
    let template = ReportTemplate {
        name: "Clone Test".to_string(),
        description: "Testing clone".to_string(),
        sections: vec![ReportSection {
            title: "Section".to_string(),
            section_type: ReportSectionType::Table,
            queries: vec![],
        }],
        format: ReportFormat::Json,
    };

    let cloned = template.clone();
    assert_eq!(template.name, cloned.name);
    assert_eq!(template.sections.len(), cloned.sections.len());
}

// ==================== ReportSection Tests ====================

#[test]
fn test_report_section_creation() {
    let section = ReportSection {
        title: "Test Section".to_string(),
        section_type: ReportSectionType::Metrics,
        queries: vec!["query1".to_string(), "query2".to_string()],
    };

    assert_eq!(section.title, "Test Section");
    assert_eq!(section.queries.len(), 2);
}

#[test]
fn test_report_section_clone() {
    let section = ReportSection {
        title: "Clone Section".to_string(),
        section_type: ReportSectionType::Recommendations,
        queries: vec!["q1".to_string()],
    };

    let cloned = section.clone();
    assert_eq!(section.title, cloned.title);
}

// ==================== ReportSectionType Tests ====================

#[test]
fn test_report_section_type_variants() {
    let summary = ReportSectionType::Summary;
    let chart = ReportSectionType::Chart;
    let table = ReportSectionType::Table;
    let metrics = ReportSectionType::Metrics;
    let recommendations = ReportSectionType::Recommendations;

    // Just verify they can be created and cloned
    let _ = summary.clone();
    let _ = chart.clone();
    let _ = table.clone();
    let _ = metrics.clone();
    let _ = recommendations.clone();
}

// ==================== ReportFormat Tests ====================

#[test]
fn test_report_format_variants() {
    let pdf = ReportFormat::Pdf;
    let html = ReportFormat::Html;
    let json = ReportFormat::Json;
    let csv = ReportFormat::Csv;
    let excel = ReportFormat::Excel;

    // Just verify they can be created and cloned
    let _ = pdf.clone();
    let _ = html.clone();
    let _ = json.clone();
    let _ = csv.clone();
    let _ = excel.clone();
}

// ==================== GeneratedReport Tests ====================

#[test]
fn test_generated_report_creation() {
    let now = Utc::now();
    let report = GeneratedReport {
        id: "report-123".to_string(),
        title: "Test Report".to_string(),
        generated_at: now,
        period_start: now - chrono::Duration::days(7),
        period_end: now,
        sections: vec![],
        summary: ReportSummary {
            total_requests: 1000,
            total_cost: 50.0,
            avg_response_time: 150.0,
            success_rate: 99.5,
            key_insights: vec!["High usage on Monday".to_string()],
            recommendations: vec!["Consider caching".to_string()],
        },
    };

    assert_eq!(report.id, "report-123");
    assert_eq!(report.summary.total_requests, 1000);
    assert_eq!(report.summary.success_rate, 99.5);
}

#[test]
fn test_generated_report_with_sections() {
    let now = Utc::now();
    let report = GeneratedReport {
        id: "report-456".to_string(),
        title: "Full Report".to_string(),
        generated_at: now,
        period_start: now - chrono::Duration::days(30),
        period_end: now,
        sections: vec![
            ReportSectionData {
                title: "Overview".to_string(),
                data: serde_json::json!({"requests": 1000}),
                charts: vec![],
            },
            ReportSectionData {
                title: "Costs".to_string(),
                data: serde_json::json!({"total": 100.0}),
                charts: vec![ChartData {
                    chart_type: "line".to_string(),
                    title: "Cost Trend".to_string(),
                    data: vec![],
                    config: serde_json::json!({}),
                }],
            },
        ],
        summary: ReportSummary {
            total_requests: 5000,
            total_cost: 250.0,
            avg_response_time: 100.0,
            success_rate: 98.0,
            key_insights: vec![],
            recommendations: vec![],
        },
    };

    assert_eq!(report.sections.len(), 2);
    assert_eq!(report.sections[1].charts.len(), 1);
}

#[test]
fn test_generated_report_clone() {
    let now = Utc::now();
    let report = GeneratedReport {
        id: "clone-test".to_string(),
        title: "Clone Report".to_string(),
        generated_at: now,
        period_start: now,
        period_end: now,
        sections: vec![],
        summary: ReportSummary {
            total_requests: 100,
            total_cost: 10.0,
            avg_response_time: 50.0,
            success_rate: 100.0,
            key_insights: vec![],
            recommendations: vec![],
        },
    };

    let cloned = report.clone();
    assert_eq!(report.id, cloned.id);
    assert_eq!(report.summary.total_requests, cloned.summary.total_requests);
}

#[test]
fn test_generated_report_serialization() {
    let now = Utc::now();
    let report = GeneratedReport {
        id: "ser-test".to_string(),
        title: "Serialization Test".to_string(),
        generated_at: now,
        period_start: now - chrono::Duration::days(1),
        period_end: now,
        sections: vec![],
        summary: ReportSummary {
            total_requests: 500,
            total_cost: 25.0,
            avg_response_time: 75.0,
            success_rate: 99.0,
            key_insights: vec!["insight1".to_string()],
            recommendations: vec!["rec1".to_string()],
        },
    };

    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["id"], "ser-test");
    assert_eq!(json["summary"]["total_requests"], 500);
    assert_eq!(json["summary"]["key_insights"][0], "insight1");
}

#[test]
fn test_generated_report_deserialization() {
    let json = r#"{
        "id": "deser-test",
        "title": "Deserialization Test",
        "generated_at": "2024-01-01T00:00:00Z",
        "period_start": "2023-12-01T00:00:00Z",
        "period_end": "2024-01-01T00:00:00Z",
        "sections": [],
        "summary": {
            "total_requests": 200,
            "total_cost": 15.5,
            "avg_response_time": 80.0,
            "success_rate": 97.5,
            "key_insights": [],
            "recommendations": []
        }
    }"#;

    let report: GeneratedReport = serde_json::from_str(json).unwrap();
    assert_eq!(report.id, "deser-test");
    assert_eq!(report.summary.total_requests, 200);
    assert_eq!(report.summary.total_cost, 15.5);
}

// ==================== ReportSectionData Tests ====================

#[test]
fn test_report_section_data_creation() {
    let section = ReportSectionData {
        title: "Test Section".to_string(),
        data: serde_json::json!({"key": "value"}),
        charts: vec![],
    };

    assert_eq!(section.title, "Test Section");
    assert!(section.charts.is_empty());
}

#[test]
fn test_report_section_data_with_charts() {
    let section = ReportSectionData {
        title: "Charts Section".to_string(),
        data: serde_json::json!({}),
        charts: vec![ChartData {
            chart_type: "bar".to_string(),
            title: "Bar Chart".to_string(),
            data: vec![
                DataPoint {
                    x: serde_json::json!("Jan"),
                    y: serde_json::json!(100),
                    metadata: None,
                },
                DataPoint {
                    x: serde_json::json!("Feb"),
                    y: serde_json::json!(150),
                    metadata: Some(serde_json::json!({"note": "high"})),
                },
            ],
            config: serde_json::json!({"color": "blue"}),
        }],
    };

    assert_eq!(section.charts.len(), 1);
    assert_eq!(section.charts[0].data.len(), 2);
}

#[test]
fn test_report_section_data_serialization() {
    let section = ReportSectionData {
        title: "Ser Section".to_string(),
        data: serde_json::json!({"requests": 100}),
        charts: vec![],
    };

    let json = serde_json::to_value(&section).unwrap();
    assert_eq!(json["title"], "Ser Section");
    assert_eq!(json["data"]["requests"], 100);
}

// ==================== ChartData Tests ====================

#[test]
fn test_chart_data_creation() {
    let chart = ChartData {
        chart_type: "pie".to_string(),
        title: "Distribution".to_string(),
        data: vec![],
        config: serde_json::json!({}),
    };

    assert_eq!(chart.chart_type, "pie");
    assert_eq!(chart.title, "Distribution");
}

#[test]
fn test_chart_data_with_points() {
    let chart = ChartData {
        chart_type: "line".to_string(),
        title: "Trend".to_string(),
        data: vec![
            DataPoint {
                x: serde_json::json!(0),
                y: serde_json::json!(10),
                metadata: None,
            },
            DataPoint {
                x: serde_json::json!(1),
                y: serde_json::json!(20),
                metadata: None,
            },
            DataPoint {
                x: serde_json::json!(2),
                y: serde_json::json!(15),
                metadata: None,
            },
        ],
        config: serde_json::json!({"smooth": true}),
    };

    assert_eq!(chart.data.len(), 3);
}

#[test]
fn test_chart_data_clone() {
    let chart = ChartData {
        chart_type: "scatter".to_string(),
        title: "Points".to_string(),
        data: vec![DataPoint {
            x: serde_json::json!(1),
            y: serde_json::json!(2),
            metadata: None,
        }],
        config: serde_json::json!({}),
    };

    let cloned = chart.clone();
    assert_eq!(chart.chart_type, cloned.chart_type);
    assert_eq!(chart.data.len(), cloned.data.len());
}

#[test]
fn test_chart_data_serialization() {
    let chart = ChartData {
        chart_type: "area".to_string(),
        title: "Area Chart".to_string(),
        data: vec![],
        config: serde_json::json!({"fill": true}),
    };

    let json = serde_json::to_value(&chart).unwrap();
    assert_eq!(json["chart_type"], "area");
    assert_eq!(json["config"]["fill"], true);
}

// ==================== DataPoint Tests ====================

#[test]
fn test_data_point_creation() {
    let point = DataPoint {
        x: serde_json::json!("2024-01-01"),
        y: serde_json::json!(100),
        metadata: None,
    };

    assert_eq!(point.x, "2024-01-01");
    assert_eq!(point.y, 100);
    assert!(point.metadata.is_none());
}

#[test]
fn test_data_point_with_metadata() {
    let point = DataPoint {
        x: serde_json::json!(1),
        y: serde_json::json!(50.5),
        metadata: Some(serde_json::json!({
            "label": "Point A",
            "color": "#ff0000"
        })),
    };

    assert!(point.metadata.is_some());
    let meta = point.metadata.as_ref().unwrap();
    assert_eq!(meta["label"], "Point A");
}

#[test]
fn test_data_point_clone() {
    let point = DataPoint {
        x: serde_json::json!("x"),
        y: serde_json::json!("y"),
        metadata: Some(serde_json::json!({})),
    };

    let cloned = point.clone();
    assert_eq!(point.x, cloned.x);
    assert_eq!(point.y, cloned.y);
}

#[test]
fn test_data_point_serialization() {
    let point = DataPoint {
        x: serde_json::json!(10),
        y: serde_json::json!(20),
        metadata: None,
    };

    let json = serde_json::to_value(&point).unwrap();
    assert_eq!(json["x"], 10);
    assert_eq!(json["y"], 20);
}

// ==================== ReportSummary Tests ====================

#[test]
fn test_report_summary_creation() {
    let summary = ReportSummary {
        total_requests: 10000,
        total_cost: 500.0,
        avg_response_time: 120.0,
        success_rate: 99.9,
        key_insights: vec![
            "Usage increased 20%".to_string(),
            "Cost decreased 10%".to_string(),
        ],
        recommendations: vec![
            "Enable caching".to_string(),
            "Use batch requests".to_string(),
        ],
    };

    assert_eq!(summary.total_requests, 10000);
    assert_eq!(summary.total_cost, 500.0);
    assert_eq!(summary.key_insights.len(), 2);
    assert_eq!(summary.recommendations.len(), 2);
}

#[test]
fn test_report_summary_empty() {
    let summary = ReportSummary {
        total_requests: 0,
        total_cost: 0.0,
        avg_response_time: 0.0,
        success_rate: 0.0,
        key_insights: vec![],
        recommendations: vec![],
    };

    assert_eq!(summary.total_requests, 0);
    assert!(summary.key_insights.is_empty());
}

#[test]
fn test_report_summary_clone() {
    let summary = ReportSummary {
        total_requests: 100,
        total_cost: 10.0,
        avg_response_time: 50.0,
        success_rate: 95.0,
        key_insights: vec!["insight".to_string()],
        recommendations: vec!["rec".to_string()],
    };

    let cloned = summary.clone();
    assert_eq!(summary.total_requests, cloned.total_requests);
    assert_eq!(summary.key_insights, cloned.key_insights);
}

#[test]
fn test_report_summary_serialization() {
    let summary = ReportSummary {
        total_requests: 5000,
        total_cost: 250.0,
        avg_response_time: 100.0,
        success_rate: 98.5,
        key_insights: vec!["High traffic".to_string()],
        recommendations: vec!["Scale up".to_string()],
    };

    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["total_requests"], 5000);
    assert_eq!(json["success_rate"], 98.5);
    assert_eq!(json["key_insights"][0], "High traffic");
}

// ==================== ReportGenerator Tests ====================

#[test]
fn test_report_generator_new() {
    let generator = ReportGenerator::new();
    assert!(!generator.templates().is_empty());
}

#[test]
fn test_report_generator_default() {
    let generator = ReportGenerator::default();
    assert!(!generator.templates().is_empty());
}

#[test]
fn test_report_generator_has_usage_summary_template() {
    let generator = ReportGenerator::new();
    let templates = generator.templates();

    assert!(templates.contains_key("usage_summary"));
    let template = templates.get("usage_summary").unwrap();
    assert_eq!(template.name, "Usage Summary Report");
}

#[test]
fn test_report_generator_template_structure() {
    let generator = ReportGenerator::new();
    let template = generator.templates().get("usage_summary").unwrap();

    assert_eq!(template.sections.len(), 2);
    assert_eq!(template.sections[0].title, "Executive Summary");
    assert_eq!(template.sections[1].title, "Cost Analysis");
}

// ==================== Integration Tests ====================

#[test]
fn test_full_report_workflow() {
    // Create a generator
    let generator = ReportGenerator::new();

    // Verify templates exist
    assert!(generator.templates().contains_key("usage_summary"));

    // Create a sample report manually
    let now = Utc::now();
    let report = GeneratedReport {
        id: uuid::Uuid::new_v4().to_string(),
        title: "Weekly Usage Report".to_string(),
        generated_at: now,
        period_start: now - chrono::Duration::days(7),
        period_end: now,
        sections: vec![
            ReportSectionData {
                title: "Executive Summary".to_string(),
                data: serde_json::json!({
                    "total_requests": 50000,
                    "unique_users": 150,
                    "top_model": "gpt-4"
                }),
                charts: vec![],
            },
            ReportSectionData {
                title: "Cost Analysis".to_string(),
                data: serde_json::json!({}),
                charts: vec![ChartData {
                    chart_type: "line".to_string(),
                    title: "Daily Cost".to_string(),
                    data: (0..7)
                        .map(|i| DataPoint {
                            x: serde_json::json!(format!("Day {}", i + 1)),
                            y: serde_json::json!(100.0 + (i as f64 * 10.0)),
                            metadata: None,
                        })
                        .collect(),
                    config: serde_json::json!({"yAxisLabel": "USD"}),
                }],
            },
        ],
        summary: ReportSummary {
            total_requests: 50000,
            total_cost: 2500.0,
            avg_response_time: 150.0,
            success_rate: 99.2,
            key_insights: vec![
                "GPT-4 usage increased 25%".to_string(),
                "Response times improved by 10%".to_string(),
            ],
            recommendations: vec![
                "Consider implementing semantic caching".to_string(),
                "Review rate limiting strategy".to_string(),
            ],
        },
    };

    // Verify the report
    assert!(!report.id.is_empty());
    assert_eq!(report.sections.len(), 2);
    assert_eq!(report.sections[1].charts.len(), 1);
    assert_eq!(report.sections[1].charts[0].data.len(), 7);
    assert_eq!(report.summary.key_insights.len(), 2);

    // Serialize and deserialize
    let json_str = serde_json::to_string(&report).unwrap();
    let parsed: GeneratedReport = serde_json::from_str(&json_str).unwrap();
    assert_eq!(report.id, parsed.id);
    assert_eq!(report.summary.total_requests, parsed.summary.total_requests);
}
