use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use futures::stream::{FuturesUnordered, StreamExt};
use urlencoding::encode;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{console, Event};
use std::collections::BTreeMap;

const REFRESH_SECONDS: i32 = 60;
const TICK_MS: u32 = 1000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub instance_status: Option<String>,
    pub health_status: Option<String>,
    pub instance_name: Option<String>,
    pub details: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvStatus {
    pub customer: String,
    pub env_name: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
    pub filename: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CustomerGroup {
    pub customer: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
    pub envs: Vec<EnvStatus>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomerChartDatum {
    pub customer: String,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoricalPoint {
    pub label: String,
    pub passed: usize,
    pub failed: usize,
    pub total: usize,
    pub filename: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OCIObject {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct OCIListResponse {
    #[serde(default)]
    pub objects: Vec<OCIObject>,
    #[serde(default)]
    pub data: Vec<OCIObject>,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Failed,
    Healthy,
}

#[derive(Clone, Copy, PartialEq)]
enum PageView {
    Dashboard,
    Detail,
    History,
}

const BASE_URL: &str = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o";

fn log(msg: &str) {
    console::log_1(&JsValue::from_str(msg));
}

fn parse_customer_env(filename: &str) -> (String, String) {
    let base_name = filename.strip_suffix(".json").unwrap_or(filename);
    let parts: Vec<&str> = base_name.split('_').collect();

    let customer = parts.get(0).unwrap_or(&"UNKNOWN").to_string();
    let env = parts.get(1).unwrap_or(&"UNKNOWN").to_uppercase();

    (customer, env)
}

fn parse_history_filename(filename: &str) -> Option<(String, String, String, String, String)> {
    let base = filename.strip_suffix(".json").unwrap_or(filename);
    let parts: Vec<&str> = base.split('_').collect();

    if parts.len() < 6 {
        return None;
    }

    let customer = parts[0].to_string();
    let servergroup = parts[1].to_string();
    let month = parts[2].to_string();
    let year = parts[3].to_string();
    let hhmm = parts[4].to_string();

    if parts[5].to_lowercase() != "health" {
        return None;
    }

    Some((customer, servergroup, month, year, hhmm))
}

fn format_history_label(month: &str, year: &str, hhmm: &str) -> String {
    format!("{} {} {}", month, year, hhmm)
}

fn matches_history_file(filename: &str, customer: &str, servergroup: &str) -> bool {
    let lower = filename.to_lowercase();
    let prefix = format!("{}_{}_", customer.to_lowercase(), servergroup.to_lowercase());
    lower.starts_with(&prefix) && lower.ends_with("_health.json")
}

fn calc_pct(ok: usize, total: usize) -> f32 {
    if total > 0 {
        (ok as f32 / total as f32) * 100.0
    } else {
        0.0
    }
}

fn group_by_customer(items: Vec<EnvStatus>, filter: Filter) -> Vec<CustomerGroup> {
    let mut grouped: BTreeMap<String, Vec<EnvStatus>> = BTreeMap::new();

    for item in items.into_iter() {
        let include = match filter {
            Filter::All => true,
            Filter::Failed => item.err > 0,
            Filter::Healthy => item.err == 0,
        };

        if include {
            grouped.entry(item.customer.clone()).or_default().push(item);
        }
    }

    let mut result = Vec::new();

    for (customer, mut envs) in grouped {
        envs.sort_by(|a, b| a.env_name.cmp(&b.env_name));

        let total: usize = envs.iter().map(|e| e.total).sum();
        let ok: usize = envs.iter().map(|e| e.ok).sum();
        let err: usize = envs.iter().map(|e| e.err).sum();

        result.push(CustomerGroup {
            customer,
            total,
            ok,
            err,
            envs,
        });
    }

    result
}

fn build_customer_chart_data(groups: &[CustomerGroup]) -> Vec<CustomerChartDatum> {
    let mut data: Vec<CustomerChartDatum> = groups
        .iter()
        .map(|g| CustomerChartDatum {
            customer: g.customer.clone(),
            total: g.total,
        })
        .collect();

    data.sort_by(|a, b| b.total.cmp(&a.total).then(a.customer.cmp(&b.customer)));
    data
}

async fn fetch_json_file(filename: &str) -> Result<Vec<HealthInstance>, String> {
    let encoded_name = encode(filename);
    let file_url = format!("{}/{}", BASE_URL, encoded_name);

    log(&format!("Fetching file: {}", file_url));

    let res = Request::get(&file_url)
        .send()
        .await
        .map_err(|e| format!("Fetch error for {}: {}", filename, e))?;

    if !res.ok() {
        return Err(format!("HTTP {} for {}", res.status(), filename));
    }

    res.json::<Vec<HealthInstance>>()
        .await
        .map_err(|e| format!("JSON parse error for {}: {}", filename, e))
}

async fn fetch_object_names() -> Result<Vec<String>, String> {
    let list_url = format!("{}/?format=json", BASE_URL);
    log(&format!("Listing URL: {}", list_url));

    let resp = Request::get(&list_url)
        .send()
        .await
        .map_err(|e| format!("List API failed: {}", e))?;

    if !resp.ok() {
        return Err(format!("List API returned HTTP {}", resp.status()));
    }

    let list_data: OCIListResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse OCI list response: {}", e))?;

    let all_objects = if !list_data.objects.is_empty() {
        list_data.objects
    } else {
        list_data.data
    };

    Ok(all_objects.into_iter().map(|obj| obj.name).collect())
}

async fn fetch_jde_health_data() -> Result<Vec<EnvStatus>, String> {
    let target_files: Vec<String> = fetch_object_names()
        .await?
        .into_iter()
        .filter(|name| name.to_lowercase().ends_with("_latest.json"))
        .collect();

    log(&format!("Matched latest files: {:?}", target_files));

    if target_files.is_empty() {
        return Err("No '_latest.json' files found in bucket listing.".to_string());
    }

    let mut fetch_tasks = FuturesUnordered::new();

    for filename in target_files {
        fetch_tasks.push(async move {
            let instances = fetch_json_file(&filename).await?;
            Ok::<(String, Vec<HealthInstance>), String>((filename, instances))
        });
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();

    while let Some(task_result) = fetch_tasks.next().await {
        match task_result {
            Ok((filename, instances)) => {
                let (customer, env_name) = parse_customer_env(&filename);

                let total = instances.len();
                let mut ok = 0usize;
                let mut err = 0usize;

                for inst in &instances {
                    let instance_status = inst
                        .instance_status
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_uppercase();

                    let health_status = inst
                        .health_status
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();

                    if instance_status == "RUNNING" && health_status == "passed" {
                        ok += 1;
                    } else {
                        err += 1;
                    }
                }

                results.push(EnvStatus {
                    customer,
                    env_name,
                    total,
                    ok,
                    err,
                    filename,
                });
            }
            Err(e) => {
                log(&e);
                errors.push(e);
            }
        }
    }

    if results.is_empty() {
        return Err(format!(
            "Could not load any environment data. Errors: {}",
            errors.join(" | ")
        ));
    }

    results.sort_by(|a, b| {
        a.customer
            .cmp(&b.customer)
            .then(a.env_name.cmp(&b.env_name))
    });

    Ok(results)
}

async fn fetch_history_data(customer: &str, servergroup: &str) -> Result<Vec<HistoricalPoint>, String> {
    let mut matching_files: Vec<String> = fetch_object_names()
        .await?
        .into_iter()
        .filter(|name| matches_history_file(name, customer, servergroup))
        .collect();

    matching_files.sort();
    matching_files.reverse();
    matching_files.truncate(15);
    matching_files.reverse();

    if matching_files.is_empty() {
        return Err(format!(
            "No historical files found for customer '{}' and servergroup '{}'",
            customer, servergroup
        ));
    }

    let mut fetch_tasks = FuturesUnordered::new();

    for filename in matching_files {
        fetch_tasks.push(async move {
            let instances = fetch_json_file(&filename).await?;
            Ok::<(String, Vec<HealthInstance>), String>((filename, instances))
        });
    }

    let mut points = Vec::new();

    while let Some(result) = fetch_tasks.next().await {
        match result {
            Ok((filename, instances)) => {
                let Some((_, _, month, year, hhmm)) = parse_history_filename(&filename) else {
                    continue;
                };

                let mut passed = 0usize;
                let mut failed = 0usize;

                for inst in &instances {
                    let instance_status = inst
                        .instance_status
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_uppercase();

                    let health_status = inst
                        .health_status
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();

                    if instance_status == "RUNNING" && health_status == "passed" {
                        passed += 1;
                    } else {
                        failed += 1;
                    }
                }

                points.push(HistoricalPoint {
                    label: format_history_label(&month, &year, &hhmm),
                    passed,
                    failed,
                    total: passed + failed,
                    filename,
                });
            }
            Err(e) => log(&e),
        }
    }

    points.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(points)
}

#[component]
fn DoughnutChart(data: Vec<CustomerChartDatum>) -> impl IntoView {
    let canvas_ref = create_node_ref::<html::Canvas>();

    create_effect(move |_| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };

        let labels = data.iter().map(|d| d.customer.clone()).collect::<Vec<_>>();
        let values = data.iter().map(|d| d.total as f64).collect::<Vec<_>>();
        let colors = vec![
            "#0ea5e9", "#f97316", "#22c55e", "#ef4444", "#8b5cf6", "#eab308",
            "#14b8a6", "#ec4899", "#6366f1", "#84cc16", "#06b6d4", "#f59e0b",
            "#10b981", "#f43f5e", "#a855f7", "#3b82f6", "#78716c", "#64748b",
        ];

        let labels_js = serde_wasm_bindgen::to_value(&labels).unwrap_or(JsValue::NULL);
        let values_js = serde_wasm_bindgen::to_value(&values).unwrap_or(JsValue::NULL);
        let colors_js = serde_wasm_bindgen::to_value(&colors).unwrap_or(JsValue::NULL);

        let chart_ctor = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("Chart"))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null());

        let Some(chart_ctor) = chart_ctor else {
            log("Chart.js is not loaded on window.Chart");
            return;
        };

        let window = web_sys::window().unwrap();
        let chart_key = JsValue::from_str("__jde_customer_chart");

        if let Ok(existing) = js_sys::Reflect::get(&window, &chart_key) {
            if !existing.is_undefined() && !existing.is_null() {
                if let Ok(destroy_fn) =
                    js_sys::Reflect::get(&existing, &JsValue::from_str("destroy"))
                {
                    if let Some(destroy) = destroy_fn.dyn_ref::<js_sys::Function>() {
                        let _ = destroy.call0(&existing);
                    }
                }
            }
        }

        let data_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&data_obj, &JsValue::from_str("labels"), &labels_js);

        let dataset = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("data"), &values_js);
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("backgroundColor"), &colors_js);
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("borderColor"), &JsValue::from_str("#ffffff"));
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("borderWidth"), &JsValue::from_f64(2.0));
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("hoverOffset"), &JsValue::from_f64(8.0));

        let datasets = js_sys::Array::new();
        datasets.push(&dataset);
        let _ = js_sys::Reflect::set(&data_obj, &JsValue::from_str("datasets"), &datasets.into());

        let font_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&font_obj, &JsValue::from_str("size"), &JsValue::from_f64(11.0));
        let _ = js_sys::Reflect::set(&font_obj, &JsValue::from_str("weight"), &JsValue::from_str("600"));

        let legend_labels = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("usePointStyle"), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("pointStyle"), &JsValue::from_str("circle"));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("boxWidth"), &JsValue::from_f64(10.0));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("boxHeight"), &JsValue::from_f64(10.0));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("padding"), &JsValue::from_f64(14.0));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("color"), &JsValue::from_str("#334155"));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("font"), &font_obj);

        let legend_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&legend_obj, &JsValue::from_str("position"), &JsValue::from_str("right"));
        let _ = js_sys::Reflect::set(&legend_obj, &JsValue::from_str("labels"), &legend_labels);

        let plugins_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&plugins_obj, &JsValue::from_str("legend"), &legend_obj);

        let options_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&options_obj, &JsValue::from_str("responsive"), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&options_obj, &JsValue::from_str("maintainAspectRatio"), &JsValue::FALSE);
        let _ = js_sys::Reflect::set(&options_obj, &JsValue::from_str("cutout"), &JsValue::from_str("65%"));
        let _ = js_sys::Reflect::set(&options_obj, &JsValue::from_str("plugins"), &plugins_obj);

        let config = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("type"), &JsValue::from_str("doughnut"));
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("data"), &data_obj);
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("options"), &options_obj);

        let args = js_sys::Array::new();
        args.push(canvas.as_ref());
        args.push(&config);

        if let Some(constructor) = chart_ctor.dyn_ref::<js_sys::Function>() {
            if let Ok(chart_instance) = js_sys::Reflect::construct(constructor, &args) {
                let _ = js_sys::Reflect::set(&window, &chart_key, &chart_instance);
            } else {
                log("Failed to construct Chart.js chart");
            }
        } else {
            log("window.Chart is not callable");
        }
    });

    view! {
        <div style="height: 220px; max-width: 420px; width: 100%; margin: 0 auto; position: relative;">
            <canvas node_ref=canvas_ref style="width: 100%; height: 100%;"></canvas>
        </div>
    }
}

#[component]
fn HistoryBarChart(data: Vec<HistoricalPoint>) -> impl IntoView {
    let canvas_ref = create_node_ref::<html::Canvas>();

    create_effect(move |_| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };

        let labels = data.iter().map(|d| d.label.clone()).collect::<Vec<_>>();
        let passed = data.iter().map(|d| d.passed as f64).collect::<Vec<_>>();
        let failed = data.iter().map(|d| d.failed as f64).collect::<Vec<_>>();

        let labels_js = serde_wasm_bindgen::to_value(&labels).unwrap_or(JsValue::NULL);
        let passed_js = serde_wasm_bindgen::to_value(&passed).unwrap_or(JsValue::NULL);
        let failed_js = serde_wasm_bindgen::to_value(&failed).unwrap_or(JsValue::NULL);

        let chart_ctor = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("Chart"))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null());

        let Some(chart_ctor) = chart_ctor else {
            log("Chart.js is not loaded on window.Chart");
            return;
        };

        let window = web_sys::window().unwrap();
        let chart_key = JsValue::from_str("__jde_history_chart");

        if let Ok(existing) = js_sys::Reflect::get(&window, &chart_key) {
            if !existing.is_undefined() && !existing.is_null() {
                if let Ok(destroy_fn) = js_sys::Reflect::get(&existing, &JsValue::from_str("destroy")) {
                    if let Some(destroy) = destroy_fn.dyn_ref::<js_sys::Function>() {
                        let _ = destroy.call0(&existing);
                    }
                }
            }
        }

        let dataset_passed = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&dataset_passed, &JsValue::from_str("label"), &JsValue::from_str("Passed"));
        let _ = js_sys::Reflect::set(&dataset_passed, &JsValue::from_str("data"), &passed_js);
        let _ = js_sys::Reflect::set(&dataset_passed, &JsValue::from_str("backgroundColor"), &JsValue::from_str("#10b981"));
        let _ = js_sys::Reflect::set(&dataset_passed, &JsValue::from_str("borderRadius"), &JsValue::from_f64(4.0));

        let dataset_failed = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&dataset_failed, &JsValue::from_str("label"), &JsValue::from_str("Failed"));
        let _ = js_sys::Reflect::set(&dataset_failed, &JsValue::from_str("data"), &failed_js);
        let _ = js_sys::Reflect::set(&dataset_failed, &JsValue::from_str("backgroundColor"), &JsValue::from_str("#ef4444"));
        let _ = js_sys::Reflect::set(&dataset_failed, &JsValue::from_str("borderRadius"), &JsValue::from_f64(4.0));

        let datasets = js_sys::Array::new();
        datasets.push(&dataset_passed);
        datasets.push(&dataset_failed);

        let data_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&data_obj, &JsValue::from_str("labels"), &labels_js);
        let _ = js_sys::Reflect::set(&data_obj, &JsValue::from_str("datasets"), &datasets.into());

        let x_ticks = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&x_ticks, &JsValue::from_str("color"), &JsValue::from_str("#475569"));

        let y_ticks = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&y_ticks, &JsValue::from_str("color"), &JsValue::from_str("#475569"));

        let x_scale = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&x_scale, &JsValue::from_str("ticks"), &x_ticks);

        let y_scale = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&y_scale, &JsValue::from_str("beginAtZero"), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&y_scale, &JsValue::from_str("ticks"), &y_ticks);

        let scales = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&scales, &JsValue::from_str("x"), &x_scale);
        let _ = js_sys::Reflect::set(&scales, &JsValue::from_str("y"), &y_scale);

        let legend = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&legend, &JsValue::from_str("position"), &JsValue::from_str("top"));

        let plugins = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&plugins, &JsValue::from_str("legend"), &legend);

        let options = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&options, &JsValue::from_str("responsive"), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&options, &JsValue::from_str("maintainAspectRatio"), &JsValue::FALSE);
        let _ = js_sys::Reflect::set(&options, &JsValue::from_str("plugins"), &plugins);
        let _ = js_sys::Reflect::set(&options, &JsValue::from_str("scales"), &scales);

        let config = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("type"), &JsValue::from_str("bar"));
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("data"), &data_obj);
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("options"), &options);

        let args = js_sys::Array::new();
        args.push(canvas.as_ref());
        args.push(&config);

        if let Some(constructor) = chart_ctor.dyn_ref::<js_sys::Function>() {
            if let Ok(chart_instance) = js_sys::Reflect::construct(constructor, &args) {
                let _ = js_sys::Reflect::set(&window, &chart_key, &chart_instance);
            }
        }
    });

    view! {
        <div style="height: 420px; width: 100%; position: relative;">
            <canvas node_ref=canvas_ref style="width: 100%; height: 100%;"></canvas>
        </div>
    }
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(Filter::All);
    let (seconds_left, set_seconds_left) = create_signal(REFRESH_SECONDS);
    let (selected_env, set_selected_env) = create_signal::<Option<EnvStatus>>(None);
    let (selected_history_env, set_selected_history_env) = create_signal::<Option<EnvStatus>>(None);
    let (page_view, set_page_view) = create_signal(PageView::Dashboard);

    let health_resource = create_resource(
        || (),
        |_| async move { fetch_jde_health_data().await },
    );

    let detail_resource = create_resource(
        move || selected_env.get(),
        |selected| async move {
            match selected {
                Some(env) => {
                    let raw_json = fetch_json_file(&env.filename).await?;
                    let pretty = serde_json::to_string_pretty(&raw_json)
                        .map_err(|e| format!("Failed to format JSON: {}", e))?;
                    Ok::<(EnvStatus, Vec<HealthInstance>, String), String>((env, raw_json, pretty))
                }
                None => Err("No environment selected.".to_string()),
            }
        },
    );

    let history_resource = create_resource(
        move || selected_history_env.get(),
        |selected| async move {
            match selected {
                Some(env) => {
                    let points = fetch_history_data(&env.customer, &env.env_name).await?;
                    Ok::<(EnvStatus, Vec<HistoricalPoint>), String>((env, points))
                }
                None => Err("No historical environment selected.".to_string()),
            }
        },
    );

    {
        let health_resource = health_resource;
        let detail_resource = detail_resource;
        let history_resource = history_resource;

        create_effect(move |_| {
            let interval = Interval::new(TICK_MS, move || {
                let current = seconds_left.get_untracked();

                if current <= 1 {
                    set_seconds_left.set(REFRESH_SECONDS);
                    health_resource.refetch();

                    if selected_env.get_untracked().is_some() {
                        detail_resource.refetch();
                    }

                    if selected_history_env.get_untracked().is_some() {
                        history_resource.refetch();
                    }
                } else {
                    set_seconds_left.set(current - 1);
                }
            });

            on_cleanup(move || drop(interval));
        });
    }

    {
        let set_selected_env = set_selected_env.clone();
        let set_selected_history_env = set_selected_history_env.clone();
        let set_page_view = set_page_view.clone();

        create_effect(move |_| {
            let window = web_sys::window().unwrap();

            let popstate_cb = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_event: Event| {
                set_selected_env.set(None);
                set_selected_history_env.set(None);
                set_page_view.set(PageView::Dashboard);
            }));

            let _ = window.add_event_listener_with_callback(
                "popstate",
                popstate_cb.as_ref().unchecked_ref(),
            );

            on_cleanup(move || {
                let _ = window.remove_event_listener_with_callback(
                    "popstate",
                    popstate_cb.as_ref().unchecked_ref(),
                );
                drop(popstate_cb);
            });
        });
    }

    let refresh_pct = move || {
        let elapsed = REFRESH_SECONDS - seconds_left.get();
        (elapsed as f32 / REFRESH_SECONDS as f32) * 100.0
    };

    view! {
        <>
            <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>

            <div
                style="
                    padding: 14px;
                    min-height: 100vh;
                    font-family: Arial, sans-serif;
                    background:
                        linear-gradient(rgba(248,250,252,0.95), rgba(248,250,252,0.95)),
                        url('./image-2.jpg') center center / 520px auto no-repeat,
                        #f8fafc;
                    background-attachment: fixed;
                "
            >
                <div style="max-width: 1800px; margin: auto;">
                    <Show
                        when=move || page_view.get() == PageView::Dashboard
                        fallback=move || {
                            if page_view.get() == PageView::Detail {
                                view! {
                                    <Transition fallback=|| view! { <p>"Loading detail..."</p> }>
                                        {move || {
                                            detail_resource.get().map(|res| match res {
                                                Err(e) => view! {
                                                    <>
                                                        <button
                                                            on:click=move |_| {
                                                                if let Some(window) = web_sys::window() {
                                                                    if let Ok(history) = window.history() {
                                                                        let _ = history.back();
                                                                    } else {
                                                                        set_selected_env.set(None);
                                                                        set_page_view.set(PageView::Dashboard);
                                                                    }
                                                                } else {
                                                                    set_selected_env.set(None);
                                                                    set_page_view.set(PageView::Dashboard);
                                                                }
                                                            }
                                                            style="margin-bottom: 12px; border: none; background: #1e293b; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                        >
                                                            "← Back to dashboard"
                                                        </button>

                                                        <div style="background: white; border-radius: 12px; padding: 16px; color: #dc2626; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                            {e}
                                                        </div>
                                                    </>
                                                }.into_view(),

                                                Ok((env, raw_json, pretty_json)) => {
                                                    let pct = calc_pct(env.ok, env.total);
                                                    let env_for_history = env.clone();

                                                    view! {
                                                        <>
                                                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; gap: 10px; flex-wrap: wrap;">
                                                                <button
                                                                    on:click=move |_| {
                                                                        if let Some(window) = web_sys::window() {
                                                                            if let Ok(history) = window.history() {
                                                                                let _ = history.back();
                                                                            } else {
                                                                                set_selected_env.set(None);
                                                                                set_page_view.set(PageView::Dashboard);
                                                                            }
                                                                        } else {
                                                                            set_selected_env.set(None);
                                                                            set_page_view.set(PageView::Dashboard);
                                                                        }
                                                                    }
                                                                    style="border: none; background: #1e293b; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                                >
                                                                    "← Back to dashboard"
                                                                </button>

                                                                <div style="display: flex; gap: 8px; flex-wrap: wrap;">
                                                                    <button
                                                                        on:click=move |_| detail_resource.refetch()
                                                                        style="border: none; background: #2563eb; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                                    >
                                                                        "Refresh selected env"
                                                                    </button>

                                                                    <button
                                                                        on:click=move |_| {
                                                                            if let Some(window) = web_sys::window() {
                                                                                if let Ok(history) = window.history() {
                                                                                    let _ = history.push_state_with_url(
                                                                                        &JsValue::NULL,
                                                                                        "",
                                                                                        Some("#history"),
                                                                                    );
                                                                                }
                                                                            }
                                                                            set_selected_history_env.set(Some(env_for_history.clone()));
                                                                            set_page_view.set(PageView::History);
                                                                        }
                                                                        style="border: none; background: #0f766e; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                                    >
                                                                        "View history"
                                                                    </button>
                                                                </div>
                                                            </div>

                                                            <div style="background: white; border-radius: 12px; padding: 16px; margin-bottom: 14px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                                <div style="color: #94a3b8; font-size: 0.68rem; font-weight: 800; text-transform: uppercase;">
                                                                    {env.customer.clone()}
                                                                </div>

                                                                <div style="color: #0f172a; font-size: 1.35rem; font-weight: 900; margin: 6px 0 10px 0;">
                                                                    {env.env_name.clone()}
                                                                </div>

                                                                <div style="display: flex; gap: 12px; flex-wrap: wrap; color: #475569; font-size: 0.82rem; margin-bottom: 12px;">
                                                                    <div>{format!("Total: {}", env.total)}</div>
                                                                    <div>{format!("OK: {}", env.ok)}</div>
                                                                    <div>{format!("Error: {}", env.err)}</div>
                                                                    <div>{format!("Health: {:.1}%", pct)}</div>
                                                                    <div>{format!("Source: {}", env.filename)}</div>
                                                                </div>

                                                                <div style="background: #e2e8f0; height: 8px; border-radius: 999px; overflow: hidden;">
                                                                    <div style=format!(
                                                                        "height: 100%; width: {:.2}%; background: {}; transition: width 0.4s;",
                                                                        pct,
                                                                        if env.err == 0 { "#10b981" } else { "#ef4444" }
                                                                    )></div>
                                                                </div>
                                                            </div>

                                                            <div style="background: white; border-radius: 12px; padding: 16px; margin-bottom: 14px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                                <div style="font-weight: 800; margin-bottom: 10px; color: #0f172a;">
                                                                    "Health Cheails"
                                                                </div>

                                                                <div style="display: grid; gap: 10px;">
                                                                    {
                                                                        raw_json
                                                                            .iter()
                                                                            .map(|item| {
                                                                                let instance_label = item
                                                                                    .instance_name
                                                                                    .clone()
                                                                                    .unwrap_or_else(|| "-".to_string());

                                                                                view! {
                                                                                    <details style="border: 1px solid #e2e8f0; border-radius: 10px; background: #f8fafc; overflow: hidden;">
                                                                                        <summary style="cursor: pointer; list-style: none; padding: 12px; font-size: 0.82rem; font-weight: 800; color: #0f172a; display: flex; align-items: center; justify-content: space-between;">
                                                                                            <span>{instance_label.clone()}</span>
                                                                                            <span style="font-size: 0.70rem; color: #64748b; font-weight: 700;">"Click to expand"</span>
                                                                                        </summary>

                                                                                        <div style="padding: 0 12px 12px 12px;">
                                                                                            <table style="width: 100%; border-collapse: collapse; background: white; border: 1px solid #e2e8f0; border-radius: 8px; overflow: hidden;">
                                                                                                <tbody>
                                                                                                    <tr>
                                                                                                        <td style="padding: 10px; border-bottom: 1px solid #e2e8f0; width: 180px; font-weight: 800; color: #334155; background: #f8fafc;">
                                                                                                            "Instance Name"
                                                                                                        </td>
                                                                                                        <td style="padding: 10px; border-bottom: 1px solid #e2e8f0; color: #334155;">
                                                                                                            {item.instance_name.clone().unwrap_or_else(|| "-".to_string())}
                                                                                                        </td>
                                                                                                    </tr>
                                                                                                    <tr>
                                                                                                        <td style="padding: 10px; border-bottom: 1px solid #e2e8f0; font-weight: 800; color: #334155; background: #f8fafc;">
                                                                                                            "Instance Status"
                                                                                                        </td>
                                                                                                        <td style="padding: 10px; border-bottom: 1px solid #e2e8f0; color: #334155;">
                                                                                                            {item.instance_status.clone().unwrap_or_else(|| "-".to_string())}
                                                                                                        </td>
                                                                                                    </tr>
                                                                                                    <tr>
                                                                                                        <td style="padding: 10px; border-bottom: 1px solid #e2e8f0; font-weight: 800; color: #334155; background: #f8fafc;">
                                                                                                            "Health Status"
                                                                                                        </td>
                                                                                                        <td style="padding: 10px; border-bottom: 1px solid #e2e8f0; color: #334155;">
                                                                                                            {item.health_status.clone().unwrap_or_else(|| "-".to_string())}
                                                                                                        </td>
                                                                                                    </tr>
                                                                                                    <tr>
                                                                                                        <td style="padding: 10px; font-weight: 800; color: #334155; background: #f8fafc;">
                                                                                                            "Details"
                                                                                                        </td>
                                                                                                        <td style="padding: 10px; color: #334155;">
                                                                                                            {item.details.clone().unwrap_or_else(|| "-".to_string())}
                                                                                                        </td>
                                                                                                    </tr>
                                                                                                </tbody>
                                                                                            </table>
                                                                                        </div>
                                                                                    </details>
                                                                                }
                                                                            })
                                                                            .collect_view()
                                                                    }
                                                                </div>
                                                            </div>

                                                            <div style="background: #0f172a; color: #e2e8f0; border-radius: 12px; padding: 16px; box-shadow: 0 6px 18px rgba(0,0,0,0.12);">
                                                                <div style="font-weight: 800; margin-bottom: 10px; color: #f8fafc;">
                                                                    "Raw JSON"
                                                                </div>
                                                                <pre style="margin: 0; white-space: pre-wrap; word-break: break-word; font-size: 0.78rem; line-height: 1.42; overflow-x: auto;">
                                                                    {pretty_json}
                                                                </pre>
                                                            </div>
                                                        </>
                                                    }.into_view()
                                                }
                                            })
                                        }}
                                    </Transition>
                                }.into_view()
                            } else {
                                view! {
                                    <Transition fallback=|| view! { <p>"Loading historical data..."</p> }>
                                        {move || {
                                            history_resource.get().map(|res| match res {
                                                Err(e) => view! {
                                                    <>
                                                        <button
                                                            on:click=move |_| {
                                                                if let Some(window) = web_sys::window() {
                                                                    if let Ok(history) = window.history() {
                                                                        let _ = history.back();
                                                                    } else {
                                                                        set_selected_history_env.set(None);
                                                                        set_page_view.set(PageView::Dashboard);
                                                                    }
                                                                } else {
                                                                    set_selected_history_env.set(None);
                                                                    set_page_view.set(PageView::Dashboard);
                                                                }
                                                            }
                                                            style="margin-bottom: 12px; border: none; background: #1e293b; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                        >
                                                            "← Back to dashboard"
                                                        </button>

                                                        <div style="background: white; border-radius: 12px; padding: 16px; color: #dc2626; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                            {e}
                                                        </div>
                                                    </>
                                                }.into_view(),

                                                Ok((env, points)) => {
                                                    let total_passed: usize = points.iter().map(|p| p.passed).sum();
                                                    let total_failed: usize = points.iter().map(|p| p.failed).sum();
                                                    let sample_count = points.len();

                                                    view! {
                                                        <>
                                                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; gap: 10px; flex-wrap: wrap;">
                                                                <button
                                                                    on:click=move |_| {
                                                                        if let Some(window) = web_sys::window() {
                                                                            if let Ok(history) = window.history() {
                                                                                let _ = history.back();
                                                                            } else {
                                                                                set_selected_history_env.set(None);
                                                                                set_page_view.set(PageView::Dashboard);
                                                                            }
                                                                        } else {
                                                                            set_selected_history_env.set(None);
                                                                            set_page_view.set(PageView::Dashboard);
                                                                        }
                                                                    }
                                                                    style="border: none; background: #1e293b; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                                >
                                                                    "← Back to dashboard"
                                                                </button>

                                                                <button
                                                                    on:click=move |_| history_resource.refetch()
                                                                    style="border: none; background: #2563eb; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                                >
                                                                    "Refresh history"
                                                                </button>
                                                            </div>

                                                            <div style="background: white; border-radius: 12px; padding: 16px; margin-bottom: 14px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                                <div style="color: #94a3b8; font-size: 0.68rem; font-weight: 800; text-transform: uppercase;">
                                                                    {env.customer.clone()}
                                                                </div>

                                                                <div style="color: #0f172a; font-size: 1.35rem; font-weight: 900; margin: 6px 0 10px 0;">
                                                                    {format!("{} Historical Health", env.env_name)}
                                                                </div>

                                                                <div style="display: flex; gap: 12px; flex-wrap: wrap; color: #475569; font-size: 0.82rem; margin-bottom: 12px;">
                                                                    <div>{format!("Samples: {}", sample_count)}</div>
                                                                    <div>{format!("Passed: {}", total_passed)}</div>
                                                                    <div>{format!("Failed: {}", total_failed)}</div>
                                                                    <div>"Showing latest 15 files"</div>
                                                                </div>

                                                                <div style="display: flex; gap: 10px; flex-wrap: wrap;">
                                                                    <div style="display: flex; align-items: center; gap: 6px; font-size: 0.78rem; color: #334155;">
                                                                        <span style="display: inline-block; width: 10px; height: 10px; background: #10b981; border-radius: 2px;"></span>
                                                                        <span>"Passed"</span>
                                                                    </div>
                                                                    <div style="display: flex; align-items: center; gap: 6px; font-size: 0.78rem; color: #334155;">
                                                                        <span style="display: inline-block; width: 10px; height: 10px; background: #ef4444; border-radius: 2px;"></span>
                                                                        <span>"Failed"</span>
                                                                    </div>
                                                                </div>
                                                            </div>

                                                            <div style="background: white; border-radius: 12px; padding: 16px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                                <HistoryBarChart data=points.clone() />
                                                            </div>
                                                        </>
                                                    }.into_view()
                                                }
                                            })
                                        }}
                                    </Transition>
                                }.into_view()
                            }
                        }
                    >
                        <div style="display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; gap: 14px; margin-bottom: 14px;">
                            <div style="justify-self: start;">
                                <div style="display: flex; gap: 4px; background: #f1f5f9; padding: 4px; border-radius: 8px; width: fit-content;">
                                    <button
                                        on:click=move |_| set_filter.set(Filter::All)
                                        style=move || format!(
                                            "border: none; padding: 6px 12px; border-radius: 6px; cursor: pointer; font-weight: 700; font-size: 0.84rem; background: {}; color: {};",
                                            if filter.get() == Filter::All { "#1e293b" } else { "transparent" },
                                            if filter.get() == Filter::All { "white" } else { "#64748b" }
                                        )
                                    >
                                        "ALL"
                                    </button>

                                    <button
                                        on:click=move |_| set_filter.set(Filter::Failed)
                                        style=move || format!(
                                            "border: none; padding: 6px 12px; border-radius: 6px; cursor: pointer; font-weight: 700; font-size: 0.84rem; background: {}; color: {};",
                                            if filter.get() == Filter::Failed { "#ef4444" } else { "transparent" },
                                            if filter.get() == Filter::Failed { "white" } else { "#64748b" }
                                        )
                                    >
                                        "FAILED"
                                    </button>

                                    <button
                                        on:click=move |_| set_filter.set(Filter::Healthy)
                                        style=move || format!(
                                            "border: none; padding: 6px 12px; border-radius: 6px; cursor: pointer; font-weight: 700; font-size: 0.84rem; background: {}; color: {};",
                                            if filter.get() == Filter::Healthy { "#10b981" } else { "transparent" },
                                            if filter.get() == Filter::Healthy { "white" } else { "#64748b" }
                                        )
                                    >
                                        "HEALTHY"
                                    </button>
                                </div>
                            </div>

                            <div style="justify-self: center;">
                                <h2 style="margin: 0; color: #1a39ea; font-weight: 900; letter-spacing: 0.3px; font-size: 1.1rem; text-align: center;">
                                    "JDE Environment Health Dashboard"
                                </h2>
                            </div>

                            <div style="justify-self: end;">
                                <div style="display: flex; align-items: center; gap: 10px; flex-wrap: wrap; justify-content: flex-end;">
                                    <div style="min-width: 220px;">
                                        <div style="display: flex; justify-content: space-between; margin-bottom: 5px; font-size: 0.74rem; color: #64748b; font-weight: 700;">
                                            <span>"Auto refresh"</span>
                                            <span>{move || format!("{}s", seconds_left.get())}</span>
                                        </div>

                                        <div style="background: #e2e8f0; height: 8px; border-radius: 999px; overflow: hidden;">
                                            <div style=move || format!(
                                                "height: 100%; width: {:.2}%; background: #2563eb; transition: width 1s linear;",
                                                refresh_pct()
                                            )></div>
                                        </div>
                                    </div>

                                    <button
                                        on:click=move |_| {
                                            set_seconds_left.set(REFRESH_SECONDS);
                                            health_resource.refetch();
                                        }
                                        style="border: none; background: #2563eb; color: white; padding: 9px 13px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                    >
                                        "Refresh now"
                                    </button>
                                </div>
                            </div>
                        </div>

                        <Transition fallback=|| view! { <p>"Processing..."</p> }>
                            {move || {
                                health_resource.get().map(|res| match res {
                                    Err(e) => view! {
                                        <div style="color: #ef4444; padding: 16px; background: white; border-radius: 8px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                            <div style="font-weight: 700; margin-bottom: 6px;">"Load failed"</div>
                                            <div>{e}</div>
                                        </div>
                                    }.into_view(),

                                    Ok(items) => {
                                        let filtered_for_summary: Vec<EnvStatus> = items
                                            .iter()
                                            .cloned()
                                            .filter(|item| match filter.get() {
                                                Filter::All => true,
                                                Filter::Failed => item.err > 0,
                                                Filter::Healthy => item.err == 0,
                                            })
                                            .collect();

                                        let total_ok: usize = filtered_for_summary.iter().map(|i| i.ok).sum();
                                        let total_inst: usize = filtered_for_summary.iter().map(|i| i.total).sum();
                                        let total_err: usize = filtered_for_summary.iter().map(|i| i.err).sum();

                                        let total_customers: usize = {
                                            let mut s = BTreeMap::new();
                                            for item in &filtered_for_summary {
                                                s.insert(item.customer.clone(), true);
                                            }
                                            s.len()
                                        };

                                        let health_pct = calc_pct(total_ok, total_inst);
                                        let customer_groups = group_by_customer(items, filter.get());
                                        let chart_data = build_customer_chart_data(&customer_groups);

                                        view! {
                                            <>
                                                <div style="display: grid; grid-template-columns: minmax(320px, 420px) 1fr; gap: 12px; margin-bottom: 14px;">
                                                    <div style="background: white; border-radius: 12px; padding: 14px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; gap: 8px; flex-wrap: wrap;">
                                                            <div>
                                                                <div style="color: #94a3b8; font-size: 0.68rem; font-weight: 800; text-transform: uppercase;">
                                                                    "Customer Distribution"
                                                                </div>
                                                                <div style="color: #0f172a; font-size: 0.92rem; font-weight: 900; margin-top: 4px;">
                                                                    "Instances by customer"
                                                                </div>
                                                            </div>
                                                            <div style="font-size: 0.72rem; color: #64748b; font-weight: 700;">
                                                                {format!("{} customers", total_customers)}
                                                            </div>
                                                        </div>

                                                        <DoughnutChart data=chart_data />
                                                    </div>

                                                    <div style="display: grid; grid-template-rows: auto 1fr; gap: 10px;">
                                                        <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                                                                <span style="font-weight: 800; color: #1e293b; font-size: 0.82rem;">"OVERALL HEALTH"</span>
                                                                <span style=format!(
                                                                    "font-weight: 900; font-size: 0.92rem; color: {};",
                                                                    if health_pct > 90.0 { "#10b981" } else { "#ef4444" }
                                                                )>
                                                                    {format!("{:.1}%", health_pct)}
                                                                </span>
                                                            </div>

                                                            <div style="background: #f1f5f9; height: 8px; border-radius: 999px; overflow: hidden;">
                                                                <div style=format!(
                                                                    "background: {}; height: 100%; width: {:.2}%; transition: width 0.4s;",
                                                                    if health_pct > 90.0 { "#10b981" } else { "#ef4444" },
                                                                    health_pct
                                                                )></div>
                                                            </div>

                                                            <div style="margin-top: 8px; font-size: 0.72rem; color: #64748b;">
                                                                {format!("{} healthy out of {} instances", total_ok, total_inst)}
                                                            </div>
                                                        </div>

                                                        <div style="display: grid; grid-template-columns: repeat(4, minmax(150px, 1fr)); gap: 10px;">
                                                            <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                                <div style="color: #94a3b8; font-size: 0.64rem; font-weight: 800; text-transform: uppercase;">"Customers"</div>
                                                                <div style="font-size: 1.25rem; font-weight: 900; color: #0f172a; margin-top: 6px;">{total_customers}</div>
                                                            </div>

                                                            <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                                <div style="color: #94a3b8; font-size: 0.64rem; font-weight: 800; text-transform: uppercase;">"Instances"</div>
                                                                <div style="font-size: 1.25rem; font-weight: 900; color: #0f172a; margin-top: 6px;">{total_inst}</div>
                                                            </div>

                                                            <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                                <div style="color: #94a3b8; font-size: 0.64rem; font-weight: 800; text-transform: uppercase;">"Healthy"</div>
                                                                <div style="font-size: 1.25rem; font-weight: 900; color: #10b981; margin-top: 6px;">{total_ok}</div>
                                                            </div>

                                                            <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                                <div style="color: #94a3b8; font-size: 0.64rem; font-weight: 800; text-transform: uppercase;">"Errors"</div>
                                                                <div style="font-size: 1.25rem; font-weight: 900; color: #ef4444; margin-top: 6px;">{total_err}</div>
                                                            </div>
                                                        </div>
                                                    </div>
                                                </div>

                                                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 10px; align-items: stretch;">
                                                    {
                                                        customer_groups
                                                            .into_iter()
                                                            .map(|group| {
                                                                let customer_pct = calc_pct(group.ok, group.total);
                                                                let customer_healthy = group.err == 0;
                                                                let max_bar_value = group.envs.iter().map(|e| e.total).max().unwrap_or(1);

                                                                view! {
                                                                    <div style="background: #ffffff; border-radius: 12px; padding: 10px; box-shadow: 0 1px 4px rgba(0,0,0,0.08); min-height: 220px; display: flex; flex-direction: column;">
                                                                        <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 8px; margin-bottom: 8px;">
                                                                            <div>
                                                                                <div style="color: #94a3b8; font-size: 0.54rem; font-weight: 800; text-transform: uppercase; margin-bottom: 2px;">
                                                                                    "CUSTOMER"
                                                                                </div>
                                                                                <div style="font-size: 0.88rem; font-weight: 900; color: #0f172a; line-height: 1.1;">
                                                                                    {group.customer.clone()}
                                                                                </div>
                                                                            </div>

                                                                            <div style=format!(
                                                                                "font-size: 0.60rem; font-weight: 800; padding: 3px 7px; border-radius: 999px; background: {}; color: white;",
                                                                                if customer_healthy { "#10b981" } else { "#ef4444" }
                                                                            )>
                                                                                {if customer_healthy { "HEALTHY" } else { "ISSUES" }}
                                                                            </div>
                                                                        </div>

                                                                        <div style="display: flex; align-items: end; justify-content: space-between; gap: 8px; min-height: 92px; padding: 8px 6px 6px 6px; border-radius: 10px; background: #f8fafc; border: 1px solid #e2e8f0;">
                                    {
    group.envs
        .into_iter()
        .map(|item| {
            let item_for_click = item.clone();
            let item_for_history = item.clone();

            let ok_height = if max_bar_value > 0 {
                (item.ok as f32 / max_bar_value as f32) * 60.0
            } else {
                4.0
            };

            let err_height = if max_bar_value > 0 {
                (item.err as f32 / max_bar_value as f32) * 60.0
            } else {
                1.0
            };

            view! {
                <div
                    style="flex: 1; min-width: 0; display: flex; flex-direction: column; align-items: center; justify-content: end; gap: 4px;"
                    title=format!("{} | OK: {} | ERR: {} | TOTAL: {}", item.env_name, item.ok, item.err, item.total)
                >
                    <div
                        on:click=move |_| {
                            if let Some(window) = web_sys::window() {
                                if let Ok(history) = window.history() {
                                    let _ = history.push_state_with_url(
                                        &JsValue::NULL,
                                        "",
                                        Some("#details"),
                                    );
                                }
                            }
                            set_selected_env.set(Some(item_for_click.clone()));
                            set_page_view.set(PageView::Detail);
                        }
                        style="width: 100%; display: flex; align-items: end; justify-content: center; gap: 4px; cursor: pointer;"
                    >
                        <div
                            style="
                                writing-mode: vertical-rl;
                                transform: rotate(180deg);
                                font-size: 0.56rem;
                                font-weight: 900;
                                color: #1e293b;
                                line-height: 1;
                                white-space: nowrap;
                                text-align: center;
                            "
                        >
                            {item.env_name.clone()}
                        </div>

                        <div style="height: 64px; display: flex; align-items: end; justify-content: center; gap: 4px;">
                            <div style="display: flex; flex-direction: column; align-items: center; justify-content: end; gap: 2px; width: 14px;">
                                <div style="font-size: 0.50rem; font-weight: 800; color: #10b981; line-height: 1;">
                                    {item.ok}
                                </div>
                                <div style=format!(
                                    "width: 100%; height: {:.2}px; background: #10b981; border-radius: 4px 4px 0 0; min-height: {};",
                                    ok_height,
                                    if item.ok > 0 { "4px" } else { "1px" }
                                )></div>
                            </div>

                            <div style="display: flex; flex-direction: column; align-items: center; justify-content: end; gap: 2px; width: 14px;">
                                <div style="font-size: 0.50rem; font-weight: 800; color: #ef4444; line-height: 1;">
                                    {item.err}
                                </div>
                                <div style=format!(
                                    "width: 100%; height: {:.2}px; background: #ef4444; border-radius: 4px 4px 0 0; min-height: {};",
                                    err_height,
                                    if item.err > 0 { "4px" } else { "1px" }
                                )></div>
                            </div>
                        </div>
                    </div>

                    <div style="font-size: 0.52rem; color: #64748b; margin-top: 2px;">
                        {format!("{}/{}", item.ok, item.total)}
                    </div>

                    <button
                        on:click=move |_| {
                            if let Some(window) = web_sys::window() {
                                if let Ok(history) = window.history() {
                                    let _ = history.push_state_with_url(
                                        &JsValue::NULL,
                                        "",
                                        Some("#history"),
                                    );
                                }
                            }
                            set_selected_history_env.set(Some(item_for_history.clone()));
                            set_page_view.set(PageView::History);
                        }
                        style="margin-top: 6px; border: none; background: #2563eb; color: white; padding: 4px 8px; border-radius: 6px; cursor: pointer; font-weight: 700; font-size: 0.56rem;"
                    >
                        "History"
                    </button>
                </div>
            }
        })
        .collect_view()
}
                                                                        </div>

                                                                        <div style="display: flex; justify-content: center; gap: 10px; margin-top: 6px; font-size: 0.58rem; color: #475569;">
                                                                            <div style="display: flex; align-items: center; gap: 4px;">
                                                                                <span style="display: inline-block; width: 8px; height: 8px; background: #10b981; border-radius: 2px;"></span>
                                                                                <span>"Passed"</span>
                                                                            </div>
                                                                            <div style="display: flex; align-items: center; gap: 4px;">
                                                                                <span style="display: inline-block; width: 8px; height: 8px; background: #ef4444; border-radius: 2px;"></span>
                                                                                <span>"Failed"</span>
                                                                            </div>
                                                                        </div>

                                                                        <div style="margin-top: auto; padding-top: 8px;">
                                                                            <div style="background: #f8fafc; border-radius: 10px; padding: 8px; border: 1px solid #e2e8f0;">
                                                                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
                                                                                    <span style="font-size: 0.62rem; font-weight: 800; color: #334155;">
                                                                                        "Customer Summary"
                                                                                    </span>

                                                                                    <span style=format!(
                                                                                        "font-size: 0.68rem; font-weight: 900; color: {};",
                                                                                        if customer_healthy { "#10b981" } else { "#ef4444" }
                                                                                    )>
                                                                                        {format!("{:.1}%", customer_pct)}
                                                                                    </span>
                                                                                </div>

                                                                                <div style="background: #e2e8f0; height: 6px; border-radius: 999px; overflow: hidden;">
                                                                                    <div style=format!(
                                                                                        "height: 100%; width: {:.2}%; background: {}; transition: width 0.4s;",
                                                                                        customer_pct,
                                                                                        if customer_healthy { "#10b981" } else { "#ef4444" }
                                                                                    )></div>
                                                                                </div>

                                                                                <div style="display: flex; justify-content: space-between; margin-top: 6px; font-size: 0.58rem; color: #64748b;">
                                                                                    <span>{format!("OK {}", group.ok)}</span>
                                                                                    <span>{format!("ERR {}", group.err)}</span>
                                                                                    <span>{format!("TOTAL {}", group.total)}</span>
                                                                                </div>
                                                                            </div>
                                                                        </div>
                                                                    </div>
                                                                }
                                                            })
                                                            .collect_view()
                                                    }
                                                </div>
                                            </>
                                        }.into_view()
                                    }
                                })
                            }}
                        </Transition>
                    </Show>
                </div>
            </div>
        </>
    }
}

fn main() {
    mount_to_body(|| view! { <App /> })
}