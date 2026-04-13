use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    #[serde(default, alias = "instanceHealths")] 
    pub instances: Vec<HealthInstance>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)] pub customer_name: Option<String>,
    #[serde(default)] pub host_name: Option<String>,
    #[serde(default)] pub server_group: Option<String>, 
    #[serde(default, alias = "instanceName")] pub group: Option<String>,
    #[serde(default, alias = "healthStatus")] pub health_status: Option<String>, 
    #[serde(default, alias = "instanceStatus")] pub status: Option<String>,
    #[serde(default, alias = "executedOn")] pub last_sync: Option<String>,
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

    set_interval(
        move || { set_refresh_count.update(|n| *n += 1); },
        Duration::from_millis(300_000),
    );

    view! {
        <div style="padding: 15px; font-family: 'Segoe UI', sans-serif; background-color: #f4f7f9; min-height: 100vh;">
            <div style="max-width: 1450px; margin: auto; background: white; padding: 20px; border-radius: 12px; box-shadow: 0 8px 20px rgba(0,0,0,0.05);">
                
                <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #004488; padding-bottom: 10px; margin-bottom: 20px;">
                    <div>
                        <h2 style="color: #004488; margin: 0; font-size: 1.5em;">"JDE Global Health Monitor"</h2>
                        <p style="margin: 3px 0 0 0; font-size: 0.75em; color: #666;">"OEL 8.x Infrastructure Monitoring"</p>
                    </div>
                    <div style="background: #004488; color: white; padding: 6px 12px; border-radius: 8px; font-size: 0.8em; font-weight: bold;">
                        "Syncs: " {move || refresh_count.get()}
                    </div>
                </div>

                <Transition fallback=|| ()>
                    {move || health_data.get().map(|data| if let Ok(insts) = data {
                        let mut customers = HashSet::new();
                        let mut critical_count = 0;
                        let mut unique_servers = HashSet::new();
                        
                        for i in &insts {
                            if let Some(c) = &i.customer_name { customers.insert(c.clone()); }
                            let key = (i.host_name.clone(), i.group.clone());
                            if unique_servers.insert(key) {
                                let h_stat = i.health_status.as_deref().unwrap_or("").to_uppercase();
                                let i_stat = i.status.as_deref().unwrap_or("").to_uppercase();
                                if h_stat == "FAILED" || i_stat == "STOPPED" || i_stat == "FAILED" { 
                                    critical_count += 1; 
                                }
                            }
                        }
                        view! {
                            <div style="display: flex; gap: 15px; margin-bottom: 20px;">
                                <div on:click=move |_| { set_show_only_critical.set(false); set_selected_customer.set(None); }
                                     style="flex: 1; background: #ebf8ff; padding: 15px; border-radius: 10px; border-left: 5px solid #3182ce; cursor: pointer;">
                                    <div style="font-size: 0.75em; color: #2b6cb0; font-weight: bold;">"MANAGED CUSTOMERS"</div>
                                    <div style="font-size: 1.6em; font-weight: bold; color: #2c5282;">{customers.len()}</div>
                                </div>
                                <div on:click=move |_| { set_show_only_critical.set(true); set_selected_customer.set(None); }
                                     style="flex: 1; background: #fff5f5; padding: 15px; border-radius: 10px; border-left: 5px solid #e53e3e; cursor: pointer;">
                                    <div style="font-size: 0.75em; color: #c53030; font-weight: bold;">"CRITICAL ISSUES"</div>
                                    <div style="font-size: 1.6em; font-weight: bold; color: #9b2c2c;">{critical_count}</div>
                                </div>
                            </div>
                        }.into_view()
                    } else { view! {}.into_view() })}
                </Transition>

                {move || (selected_customer.get().is_some() || show_only_critical.get()).then(|| {
                    let title = if show_only_critical.get() { "Global Critical List".to_string() } else { format!("Customer: {}", selected_customer.get().unwrap()) };
                    view! {
                        <div style="display: flex; align-items: center; gap: 15px; margin-bottom: 15px;">
                            <button on:click=move |_| { set_selected_customer.set(None); set_show_only_critical.set(false); }
                                    style="background: #004488; color: white; border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 600;">"← Back"</button>
                            <h3 style="margin: 0; color: #333;">{title}</h3>
                        </div>
                    }
                })}

                <input type="text" placeholder="Search resources..." 
                    style="width: 100%; padding: 10px; border: 1px solid #ddd; border-radius: 8px; margin-bottom: 20px;"
                    on:input=move |ev| set_search_query.set(event_target_value(&ev)) />

                <Transition fallback=|| view! { <div>"Loading..."</div> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(insts) => {
                            let q = search_query.get().to_lowercase();
                            if show_only_critical.get() { render_critical_view(insts, q) }
                            else if let Some(c) = selected_customer.get() { render_detail_view(insts, c, q) }
                            else { render_summary_view(insts, q, set_selected_customer) }
                        },
                        Err(e) => view! { <div style="color: red;">"Error: " {e}</div> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

fn render_summary_view(instances: Vec<HealthInstance>, query: String, set_selected: WriteSignal<Option<String>>) -> View {
    let mut stats: HashMap<String, (i32, i32, i32)> = HashMap::new();
    let mut unique_check = HashSet::new();

    for i in instances {
        let name = i.customer_name.clone().unwrap_or_default();
        let key = (name.clone(), i.host_name.clone(), i.group.clone());
        if unique_check.insert(key) {
            let entry = stats.entry(name).or_insert((0, 0, 0));
            let h_stat = i.health_status.as_deref().unwrap_or("").to_uppercase();
            let i_stat = i.status.as_deref().unwrap_or("").to_uppercase();
            
            if h_stat == "FAILED" || i_stat == "STOPPED" || i_stat == "FAILED" { entry.1 += 1; }
            else if i_stat == "RUNNING" || i_stat == "PASSED" { entry.0 += 1; }
            else { entry.2 += 1; }
        }
    }

    let mut sorted: Vec<_> = stats.into_iter().filter(|(n, _)| n.to_lowercase().contains(&query)).collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 15px;">
            {sorted.into_iter().map(|(name, (ok, err, unk))| {
                let name_cl = name.clone();
                view! {
                    <div on:click=move |_| set_selected.set(Some(name_cl.clone()))
                         style=format!("padding: 15px; border: 1px solid #ddd; border-radius: 10px; cursor: pointer; border-left: 5px solid {};", if err > 0 { "#e53e3e" } else { "#38a169" })>
                        <h4 style="margin: 0 0 10px 0;">{name}</h4>
                        <div style="font-size: 0.8em; font-weight: bold;">
                            <span style="color: #38a169;">"● " {ok} " OK "</span>
                            <span style="color: #e53e3e;">"● " {err} " ERR "</span>
                            <span style="color: #718096;">"● " {unk} " UNK"</span>
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

fn render_detail_view(instances: Vec<HealthInstance>, customer: String, query: String) -> View {
    let mut filtered: Vec<_> = instances.into_iter()
        .filter(|i| i.customer_name.as_ref() == Some(&customer))
        .filter(|i| serde_json::to_string(i).unwrap_or_default().to_lowercase().contains(&query))
        .collect();
    
    // Sort by Group then Instance Name
    filtered.sort_by(|a, b| a.server_group.cmp(&b.server_group).then(a.group.cmp(&b.group)));
    render_table(filtered)
}

fn render_critical_view(instances: Vec<HealthInstance>, query: String) -> View {
    let mut crit: Vec<_> = instances.into_iter()
        .filter(|i| {
            let h = i.health_status.as_deref().unwrap_or("").to_uppercase();
            let s = i.status.as_deref().unwrap_or("").to_uppercase();
            h == "FAILED" || s == "STOPPED" || s == "FAILED"
        })
        .filter(|i| serde_json::to_string(i).unwrap_or_default().to_lowercase().contains(&query))
        .collect();
    
    crit.sort_by(|a, b| a.server_group.cmp(&b.server_group).then(a.group.cmp(&b.group)));
    render_table(crit)
}

fn render_table(insts: Vec<HealthInstance>) -> View {
    view! {
        <div style="overflow-x: auto;">
            <table style="width: 100%; border-collapse: collapse; background: white; font-size: 0.85em;">
                <thead>
                    <tr style="background: #004488; color: white; text-align: left;">
                        <th style="padding: 12px;">"Raw Resource JSON"</th>
                        <th style="padding: 12px;">"Group"</th>
                        <th style="padding: 12px;">"Managed Instance"</th>
                        <th style="padding: 12px;">"Status"</th>
                        <th style="padding: 12px;">"Last Sync"</th>
                    </tr>
                </thead>
                <tbody>
                    {insts.into_iter().map(|i| {
                        let raw = serde_json::to_string(&i).unwrap_or_default();
                        let h_stat = i.health_status.as_deref().unwrap_or("").to_uppercase();
                        let i_stat = i.status.as_deref().unwrap_or("").to_uppercase();
                        
                        let (display_status, color) = if h_stat == "FAILED" { ("FAILED", "#e53e3e") }
                                                      else if i_stat == "STOPPED" || i_stat == "FAILED" { ("STOPPED", "#e53e3e") }
                                                      else if i_stat == "RUNNING" || i_stat == "PASSED" { ("RUNNING", "#38a169") }
                                                      else { ("UNKNOWN", "#718096") };
                        view! {
                            <tr style="border-bottom: 1px solid #eee;">
                                <td style="padding: 10px; font-family: monospace; font-size: 0.75em; color: #666; max-width: 400px; word-break: break-all;">{raw}</td>
                                <td style="padding: 10px; font-weight: bold;">{i.server_group.clone().unwrap_or_default()}</td>
                                <td style="padding: 10px;">{i.group.clone().unwrap_or_default()}</td>
                                <td style="padding: 10px;">
                                    <span style=format!("padding: 3px 8px; border-radius: 4px; color: white; background: {}; font-weight: bold; font-size: 0.8em;", color)>
                                        {display_status}
                                    </span>
                                </td>
                                <td style="padding: 10px; color: #888;">{i.last_sync.clone().unwrap_or_default()}</td>
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
    
    // Support both direct arrays and JDE-style wrapper objects
    if let Ok(wrapper) = serde_json::from_str::<HealthResponse>(&text) {
        return Ok(wrapper.instances);
    }
    serde_json::from_str::<Vec<HealthInstance>>(&text).map_err(|e| e.to_string())
}

fn main() { mount_to_body(|| view! { <App /> }) }