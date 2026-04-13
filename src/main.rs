use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)] pub customer_name: Option<String>,
    #[serde(default)] pub host_name: Option<String>,
    #[serde(default, alias = "instance_name")] pub group: Option<String>,
    #[serde(default)] pub status: Option<String>,
    #[serde(default, alias = "timestamp")] pub last_sync: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (search_query, set_search_query) = create_signal(String::new());
    let (selected_customer, set_selected_customer) = create_signal(None::<String>);
    let (refresh_count, set_refresh_count) = create_signal(0);
    
    let health_data = create_resource(
        move || refresh_count.get(), 
        |_| async move { fetch_health_data().await }
    );

    set_interval(
        move || { set_refresh_count.update(|n| *n += 1); },
        Duration::from_millis(300_000),
    );

    view! {
        <div style="padding: 20px; font-family: 'Segoe UI', sans-serif; background-color: #f4f7f9; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 30px; border-radius: 16px; box-shadow: 0 10px 25px rgba(0,0,0,0.05);">
                
                <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 3px solid #004488; padding-bottom: 15px; margin-bottom: 25px;">
                    <h2 style="color: #004488; margin: 0;">"JDE Global Health Monitor Dashboard"</h2>
                    <div style="background: #eef2f7; padding: 5px 12px; border-radius: 20px; font-size: 0.8em; font-weight: bold;">
                        "Sync Count: " {move || refresh_count.get()}
                    </div>
                </div>

                {move || selected_customer.get().map(|name| {
                    let back_name = name.clone();
                    view! {
                        <button 
                            on:click=move |_| {
                                set_selected_customer.set(None);
                                set_search_query.set(String::new()); 
                            }
                            style="background: #004488; color: white; border: none; padding: 10px 20px; border-radius: 8px; cursor: pointer; margin-bottom: 25px; font-weight: 600;"
                        >
                            "← Back to Overview"
                        </button>
                        <h3 style="margin-bottom: 20px;">"Customer Detail: " {back_name}</h3>
                    }
                })}

                <input 
                    type="text" 
                    placeholder="Search by Customer or Instance..." 
                    style="width: 100%; padding: 14px 20px; border: 2px solid #e1e8ed; border-radius: 12px; margin-bottom: 30px;"
                    on:input=move |ev| set_search_query.set(event_target_value(&ev))
                    prop:value=search_query
                />

                <Transition fallback=move || view! { <div style="text-align: center; padding: 40px;">"Gathering metrics..."</div> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(instances) => {
                            let query = search_query.get().to_lowercase();
                            if let Some(customer) = selected_customer.get() {
                                render_detail_view(instances, customer, query)
                            } else {
                                render_summary_view(instances, query, set_selected_customer, set_search_query)
                            }
                        },
                        Err(e) => view! { <div style="color: #c53030; padding: 20px; border: 1px solid #feb2b2; border-radius: 8px;">"Data Error: " {e}</div> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

fn render_summary_view(
    instances: Vec<HealthInstance>, 
    query: String, 
    set_selected: WriteSignal<Option<String>>,
    set_search: WriteSignal<String>
) -> View {
    let mut stats: HashMap<String, (HashSet<(String, String)>, i32, i32)> = HashMap::new();
    
    for inst in instances {
        let name = inst.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        let host = inst.host_name.clone().unwrap_or_else(|| "UnknownHost".into());
        let group = inst.group.clone().unwrap_or_else(|| "UnknownGroup".into());
        let status = inst.status.as_deref().unwrap_or("UNKNOWN").to_uppercase();
        
        let entry = stats.entry(name).or_insert((HashSet::new(), 0, 0));
        if entry.0.insert((host, group)) {
            if status == "RUNNING" || status == "PASSED" { entry.1 += 1; }
            else if status == "STOPPED" || status == "FAILED" { entry.2 += 1; }
        }
    }

    let mut sorted_customers: Vec<_> = stats.into_iter()
        .filter(|(name, _)| name.to_lowercase().contains(&query))
        .collect();
    sorted_customers.sort_by(|a, b| a.0.cmp(&b.0));

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(360px, 1fr)); gap: 20px;">
            {sorted_customers.into_iter().map(|(name, (unique_set, running, critical))| {
                let total = unique_set.len() as f32;
                let running_pct = if total > 0.0 { (running as f32 / total) * 100.0 } else { 0.0 };
                let critical_pct = if total > 0.0 { (critical as f32 / total) * 100.0 } else { 0.0 };
                
                // FIXED: Create separate clones for the closure and the view
                let name_for_closure = name.clone();
                let name_for_display = name.clone();

                let chart_style = format!(
                    "width: 70px; height: 70px; border-radius: 50%; background: conic-gradient(#38a169 0% {}%, #c53030 {}% {}%, #cbd5e0 {}% 100%); display: flex; align-items: center; justify-content: center;",
                    running_pct, running_pct, running_pct + critical_pct, running_pct + critical_pct
                );

                view! {
                    <div 
                        on:click=move |_| {
                            set_selected.set(Some(name_for_closure.clone()));
                            set_search.set(String::new());
                        }
                        style=format!(
                            "padding: 24px; border: 1px solid #e1e8ed; border-radius: 16px; cursor: pointer; background: white; display: flex; align-items: center; justify-content: space-between; border-left: 6px solid {};",
                            if critical > 0 { "#c53030" } else { "#38a169" }
                        )
                    >
                        <div>
                            <h4 style="margin: 0 0 8px 0; color: #1a202c; font-size: 1.25em;">{name_for_display}</h4>
                            <div style="font-size: 0.95em; color: #4a5568;">{unique_set.len()} " Instances"</div>
                            <div style="margin-top: 8px; font-size: 0.85em; display: flex; gap: 12px;">
                                <span style="color: #38a169; font-weight: bold;">"● " {running} " OK"</span>
                                <span style="color: #c53030; font-weight: bold;">"● " {critical} " ERR"</span>
                            </div>
                        </div>
                        <div style=chart_style>
                            <div style="width: 48px; height: 48px; background: white; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 0.85em; font-weight: 800;">
                                {format!("{:.0}%", running_pct)}
                            </div>
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}
//test
fn render_detail_view(instances: Vec<HealthInstance>, customer: String, query: String) -> View {
    let mut latest_instances: HashMap<(String, String), HealthInstance> = HashMap::new();
    for inst in instances.into_iter().filter(|i| i.customer_name.as_ref() == Some(&customer)) {
        let key = (inst.host_name.clone().unwrap_or_default(), inst.group.clone().unwrap_or_default());
        latest_instances.insert(key, inst);
    }
    let filtered: Vec<_> = latest_instances.into_values()
        .filter(|inst| inst.group.as_deref().unwrap_or("").to_lowercase().contains(&query))
        .collect();

    view! {
        <div style="overflow-x: auto; border: 1px solid #e1e8ed; border-radius: 12px;">
            <table style="width: 100%; border-collapse: collapse; background: white;">
                <thead>
                    <tr style="background-color: #004488; color: white; text-align: left;">
                        <th style="padding: 16px;">"Host Name"</th>
                        <th style="padding: 16px;">"Service Instance"</th>
                        <th style="padding: 16px;">"Status"</th>
                        <th style="padding: 16px;">"Last Heartbeat"</th>
                    </tr>
                </thead>
                <tbody>
                    {filtered.into_iter().map(|inst| {
                        let status_str = inst.status.clone().unwrap_or_else(|| "UNKNOWN".into());
                        let is_ok = status_str == "RUNNING" || status_str == "Passed";
                        let (bg, fg) = if is_ok { ("#e6fffa", "#234e52") } else { ("#fff5f5", "#742a2a") };
                        view! {
                            <tr style="border-bottom: 1px solid #edf2f7;">
                                <td style="padding: 16px; font-weight: 500;">{inst.host_name.clone().unwrap_or_default()}</td>
                                <td style="padding: 16px; color: #4a5568;">{inst.group.clone().unwrap_or_default()}</td>
                                <td style="padding: 16px;">
                                    <span style=format!("padding: 6px 12px; border-radius: 6px; font-size: 0.85em; font-weight: 800; background: {}; color: {}; border: 1px solid {};", bg, fg, fg)>
                                        {status_str}
                                    </span>
                                </td>
                                <td style="padding: 16px; font-size: 0.85em; color: #718096;">{inst.last_sync.clone().unwrap_or_default()}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }.into_view()
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    let cache_buster = js_sys::Math::random();
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?v={}", cache_buster); 
    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() { return Err(format!("HTTP Status: {}", resp.status())); }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if text.is_empty() || text == "null" { return Ok(vec![]); }
    serde_json::from_str::<Vec<HealthInstance>>(&text).map_err(|e| format!("JSON Error: {}", e))
}

fn main() { mount_to_body(|| view! { <App /> }) }