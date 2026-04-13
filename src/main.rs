use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)] pub customer_name: Option<String>,
    #[serde(default)] pub host_name: Option<String>,
    #[serde(default)] pub server_group: Option<String>, // Direct group from config.txt
    #[serde(default, alias = "instance_name")] pub group: Option<String>, // Managed server name
    #[serde(default)] pub status: Option<String>,
    #[serde(default, alias = "timestamp")] pub last_sync: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (search_query, set_search_query) = create_signal(String::new());
    let (selected_customer, set_selected_customer) = create_signal(None::<String>);
    let (refresh_count, set_refresh_count) = create_signal(0);
    let (show_only_critical, set_show_only_critical) = create_signal(false);
    
    let health_data = create_resource(
        move || refresh_count.get(), 
        |_| async move { fetch_health_data().await }
    );

    // Refresh data every 5 minutes
    set_interval(
        move || { set_refresh_count.update(|n| *n += 1); },
        Duration::from_millis(300_000),
    );

    view! {
        <div style="padding: 15px; font-family: 'Segoe UI', Tahoma, sans-serif; background-color: #f4f7f9; min-height: 100vh;">
            <div style="max-width: 1400px; margin: auto; background: white; padding: 20px; border-radius: 12px; box-shadow: 0 8px 20px rgba(0,0,0,0.05);">
                
                <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #004488; padding-bottom: 10px; margin-bottom: 20px;">
                    <div>
                        <h2 style="color: #004488; margin: 0; font-size: 1.5em;">"JDE Global Health Monitor"</h2>
                        <p style="margin: 3px 0 0 0; font-size: 0.75em; color: #666;">"Auto-refreshing UI every 5 mins"</p>
                    </div>
                    <div style="background: #004488; color: white; padding: 6px 12px; border-radius: 8px; font-size: 0.8em; font-weight: bold;">
                        "Dashboard Syncs: " {move || refresh_count.get()}
                    </div>
                </div>

                <Transition fallback=|| ()>
                    {move || health_data.get().map(|data| if let Ok(insts) = data {
                        let mut unique_map = HashSet::new();
                        let mut critical_count = 0;
                        for i in &insts {
                            let key = (i.host_name.clone(), i.group.clone());
                            if unique_map.insert(key) {
                                let s = i.status.as_deref().unwrap_or("").to_uppercase();
                                if s == "STOPPED" || s == "FAILED" { critical_count += 1; }
                            }
                        }
                        view! {
                            <div style="display: flex; gap: 15px; margin-bottom: 20px;">
                                <div on:click=move |_| { set_show_only_critical.set(false); set_selected_customer.set(None); }
                                     style="flex: 1; background: #ebf8ff; padding: 15px; border-radius: 10px; border-left: 5px solid #3182ce; cursor: pointer;">
                                    <div style="font-size: 0.75em; color: #2b6cb0; font-weight: bold; text-transform: uppercase;">"Unique Managed Servers"</div>
                                    <div style="font-size: 1.6em; font-weight: bold; color: #2c5282;">{unique_map.len()}</div>
                                </div>
                                <div on:click=move |_| { set_show_only_critical.set(true); set_selected_customer.set(None); }
                                     style="flex: 1; background: #fff5f5; padding: 15px; border-radius: 10px; border-left: 5px solid #e53e3e; cursor: pointer;">
                                    <div style="font-size: 0.75em; color: #c53030; font-weight: bold; text-transform: uppercase;">"Critical Issues"</div>
                                    <div style="font-size: 1.6em; font-weight: bold; color: #9b2c2c;">{critical_count}</div>
                                </div>
                            </div>
                        }.into_view()
                    } else { view! {}.into_view() })}
                </Transition>

                {move || (selected_customer.get().is_some() || show_only_critical.get()).then(|| {
                    let title = if show_only_critical.get() { "Global Critical Managed Servers".to_string() } else { format!("Customer: {}", selected_customer.get().unwrap()) };
                    view! {
                        <div style="display: flex; align-items: center; gap: 15px; margin-bottom: 15px;">
                            <button on:click=move |_| { set_selected_customer.set(None); set_show_only_critical.set(false); set_search_query.set(String::new()); }
                                    style="background: #004488; color: white; border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 600; font-size: 0.85em;">
                                "← Back"
                            </button>
                            <h3 style="margin: 0; color: #333; font-size: 1.1em;">{title}</h3>
                        </div>
                    }
                })}

                <input type="text" placeholder="Filter by customer or server name..." 
                    style="width: 100%; padding: 10px 15px; border: 1px solid #e1e8ed; border-radius: 8px; margin-bottom: 20px; box-sizing: border-box; font-size: 0.9em;"
                    on:input=move |ev| set_search_query.set(event_target_value(&ev))
                    prop:value=search_query />

                <Transition fallback=|| view! { <div style="text-align: center; padding: 30px; color: #666;">"Syncing data..."</div> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(instances) => {
                            let query = search_query.get().to_lowercase();
                            if show_only_critical.get() { render_critical_global_view(instances, query) }
                            else if let Some(customer) = selected_customer.get() { render_detail_view(instances, customer, query) }
                            else { render_summary_view(instances, query, set_selected_customer, set_search_query) }
                        },
                        Err(e) => view! { <div style="color: #c53030; padding: 15px; border: 1px solid #feb2b2; border-radius: 6px;">"Error: " {e}</div> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

fn render_summary_view(instances: Vec<HealthInstance>, query: String, set_selected: WriteSignal<Option<String>>, set_search: WriteSignal<String>) -> View {
    let mut stats: HashMap<String, (HashMap<String, i32>, i32, i32, i32)> = HashMap::new();
    let mut unique_check: HashSet<(String, String, String)> = HashSet::new();
    
    for inst in instances {
        let name = inst.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        let host = inst.host_name.clone().unwrap_or_else(|| "UnknownHost".into());
        let managed_server = inst.group.clone().unwrap_or_else(|| "Unknown".into());
        let config_group = inst.server_group.clone().unwrap_or_else(|| "Default".into());
        let status = inst.status.as_deref().unwrap_or("UNKNOWN").to_uppercase();
        
        if unique_check.insert((name.clone(), host, managed_server)) {
            let entry = stats.entry(name).or_insert((HashMap::new(), 0, 0, 0));
            *entry.0.entry(config_group).or_insert(0) += 1; // Grouping by server_group from config.txt
            
            if status == "RUNNING" || status == "PASSED" { entry.1 += 1; }
            else if status == "STOPPED" || status == "FAILED" { entry.2 += 1; }
            else { entry.3 += 1; }
        }
    }

    let mut sorted_customers: Vec<_> = stats.into_iter()
        .filter(|(name, _)| name.to_lowercase().contains(&query))
        .collect();
    sorted_customers.sort_by(|a, b| a.0.cmp(&b.0));

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(360px, 1fr)); gap: 15px;">
            {sorted_customers.into_iter().map(|(name, (group_counts, running, critical, unknown))| {
                let total = (running + critical + unknown) as f32;
                let running_pct = if total > 0.0 { (running as f32 / total) * 100.0 } else { 0.0 };
                let critical_pct = if total > 0.0 { (critical as f32 / total) * 100.0 } else { 0.0 };
                let unknown_pct = if total > 0.0 { (unknown as f32 / total) * 100.0 } else { 0.0 };
                
                let mut groups: Vec<_> = group_counts.into_iter().collect();
                groups.sort_by(|a, b| a.0.cmp(&b.0));

                let name_cl = name.clone();
                let chart_style = format!(
                    "width: 60px; height: 60px; border-radius: 50%; background: conic-gradient(#38a169 0% {}%, #c53030 {}% {}%, #cbd5e0 {}% {}%); display: flex; align-items: center; justify-content: center; flex-shrink: 0;",
                    running_pct, running_pct, running_pct + critical_pct, running_pct + critical_pct, running_pct + critical_pct + unknown_pct
                );

                view! {
                    <div on:click=move |_| { set_selected.set(Some(name_cl.clone())); set_search.set(String::new()); }
                        style=format!("padding: 15px; border: 1px solid #e1e8ed; border-radius: 10px; cursor: pointer; background: white; display: flex; align-items: center; justify-content: space-between; border-left: 5px solid {};",
                            if critical > 0 { "#c53030" } else if unknown > 0 { "#cbd5e0" } else { "#38a169" })>
                        <div style="flex: 1; min-width: 0; padding-right: 12px;">
                            <h4 style="margin: 0 0 8px 0; color: #1a202c; font-size: 1.1em; font-weight: 700;">{name}</h4>
                            <div style="display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 12px;">
                                {groups.into_iter().map(|(g, count)| {
                                    view! { <span style="background: #edf2f7; color: #2d3748; padding: 3px 8px; border-radius: 5px; font-size: 0.75em; font-weight: 700;">{format!("{}: {}", g, count)}</span> }
                                }).collect_view()}
                            </div>
                            <div style="font-size: 0.75em; display: flex; gap: 10px; border-top: 1px solid #f0f0f0; padding-top: 5px;">
                                <span style="color: #38a169; font-weight: 800;">"● " {running} " OK"</span>
                                <span style="color: #c53030; font-weight: 800;">"● " {critical} " ERR"</span>
                                {if unknown > 0 { view! { <span style="color: #718096; font-weight: 800;">"● " {unknown} " UNK"</span> }.into_view() } else { view! {}.into_view() }}
                            </div>
                        </div>
                        <div style=chart_style>
                            <div style="width: 42px; height: 42px; background: white; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 0.85em; font-weight: 900; color: #333;">{format!("{:.0}%", running_pct)}</div>
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

fn render_critical_global_view(instances: Vec<HealthInstance>, query: String) -> View {
    let mut critical_list: Vec<HealthInstance> = Vec::new();
    let mut seen = HashSet::new();
    for inst in instances {
        let status = inst.status.as_deref().unwrap_or("").to_uppercase();
        if status == "STOPPED" || status == "FAILED" {
            let key = (inst.host_name.clone().unwrap_or_default(), inst.group.clone().unwrap_or_default());
            if seen.insert(key) { critical_list.push(inst); }
        }
    }
    let filtered: Vec<_> = critical_list.into_iter().filter(|inst| serde_json::to_string(&inst).unwrap_or_default().to_lowercase().contains(&query)).collect();
    render_table(filtered)
}

fn render_detail_view(instances: Vec<HealthInstance>, customer: String, query: String) -> View {
    let mut latest_instances: HashMap<(String, String), HealthInstance> = HashMap::new();
    for inst in instances.into_iter().filter(|i| i.customer_name.as_ref() == Some(&customer)) {
        let key = (inst.host_name.clone().unwrap_or_default(), inst.group.clone().unwrap_or_default());
        latest_instances.insert(key, inst);
    }
    let filtered: Vec<_> = latest_instances.into_values().filter(|inst| serde_json::to_string(&inst).unwrap_or_default().to_lowercase().contains(&query)).collect();
    render_table(filtered)
}

fn render_table(filtered: Vec<HealthInstance>) -> View {
    view! {
        <div style="overflow-x: auto; border: 1px solid #e1e8ed; border-radius: 8px;">
            <table style="width: 100%; border-collapse: collapse; background: white; font-size: 0.85em;">
                <thead>
                    <tr style="background-color: #004488; color: white; text-align: left;">
                        <th style="padding: 12px;">"Raw Resource JSON"</th>
                        <th style="padding: 12px;">"Managed Server"</th>
                        <th style="padding: 12px;">"Status"</th>
                        <th style="padding: 12px;">"Last Sync"</th>
                    </tr>
                </thead>
                <tbody>
                    {filtered.into_iter().map(|inst| {
                        let status_str = inst.status.clone().unwrap_or_else(|| "UNKNOWN".into());
                        let status_upper = status_str.to_uppercase();
                        let (bg, fg) = if status_upper == "RUNNING" || status_upper == "PASSED" { ("#e6fffa", "#234e52") } 
                                       else if status_upper == "STOPPED" || status_upper == "FAILED" { ("#fff5f5", "#742a2a") }
                                       else { ("#edf2f7", "#4a5568") };
                        view! {
                            <tr style="border-bottom: 1px solid #edf2f7;">
                                <td style="padding: 12px; font-family: monospace; word-break: break-all; max-width: 500px; color: #444;">{serde_json::to_string(&inst).unwrap_or_default()}</td>
                                <td style="padding: 12px; color: #4a5568;">{inst.group.clone().unwrap_or_default()}</td>
                                <td style="padding: 12px;"><span style=format!("padding: 4px 10px; border-radius: 4px; font-size: 0.8em; font-weight: 800; background: {}; color: {}; border: 1px solid {};", bg, fg, fg)>{status_str}</span></td>
                                <td style="padding: 12px; color: #718096; font-family: monospace;">{inst.last_sync.clone().unwrap_or_default()}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }.into_view()
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?v={}", js_sys::Math::random()); 
    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if text.is_empty() || text == "null" { return Ok(vec![]); }
    serde_json::from_str::<Vec<HealthInstance>>(&text).map_err(|e| format!("JSON Error: {}", e))
}

fn main() { mount_to_body(|| view! { <App /> }) }