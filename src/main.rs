use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

// 1. DATA MODELS - Strict mapping to JDE JSON fields
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)] pub customer_name: Option<String>,
    #[serde(default)] pub host_name: Option<String>,
    #[serde(default, alias = "serverGroup")] pub server_group: Option<String>, 
    #[serde(default, alias = "instanceName")] pub instance_name: Option<String>,
    #[serde(default, alias = "healthStatus")] pub health_status: Option<String>, 
    #[serde(default, alias = "instanceStatus")] pub instance_status: Option<String>,
    #[serde(default, alias = "executedOn")] pub last_sync: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (search_query, set_search_query) = create_signal(String::new());
    let (selected_customer, set_selected_customer) = create_signal(None::<String>);
    let (refresh_count, set_refresh_count) = create_signal(0);
    
    // Resource for fetching live data
    let health_data = create_resource(
        move || refresh_count.get(), 
        |_| async move { fetch_health_data().await }
    );

    // Auto-refresh every minute for better responsiveness
    set_interval(
        move || { set_refresh_count.update(|n| *n += 1); },
        Duration::from_millis(60_000),
    );

    view! {
        <div style="padding: 20px; font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; background: #f8fafc; min-height: 100vh; color: #1e293b;">
            <div style="max-width: 1400px; margin: auto; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 4px 6px -1px rgb(0 0 0 / 0.1);">
                
                // HEADER SECTION
                <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #004488; padding-bottom: 15px; margin-bottom: 25px;">
                    <div>
                        <h1 style="color: #004488; margin: 0; font-size: 1.8em;">"JDE Global Health Monitor"</h1>
                        <p style="margin: 5px 0 0 0; color: #64748b; font-size: 0.9em;">"Real-time Multi-Tenancy Status Dashboard"</p>
                    </div>
                    <div style="text-align: right;">
                        <span style="background: #004488; color: white; padding: 6px 12px; border-radius: 20px; font-size: 0.8em; font-weight: 600;">
                            "Cycles: " {move || refresh_count.get()}
                        </span>
                    </div>
                </div>

                // TOP LEVEL STATS (Active Filters applied here)
                <Transition fallback=|| view! { <p>"Loading metrics..."</p> }>
                    {move || health_data.get().map(|data| if let Ok(insts) = data {
                        let total_customers = insts.iter().filter_map(|i| i.customer_name.as_ref()).collect::<HashSet<_>>().len();
                        let mut critical = 0;
                        for i in &insts {
                            let h = i.health_status.as_deref().unwrap_or("").to_uppercase();
                            let s = i.instance_status.as_deref().unwrap_or("").to_uppercase();
                            if h == "FAILED" || s == "STOPPED" || s == "FAILED" { critical += 1; }
                        }
                        view! {
                            <div style="display: flex; gap: 20px; margin-bottom: 25px;">
                                <div style="flex: 1; background: #f0f9ff; border-left: 6px solid #0ea5e9; padding: 20px; border-radius: 8px;">
                                    <div style="font-size: 0.8em; font-weight: 700; color: #0369a1; text-transform: uppercase;">"Unique Tenancies"</div>
                                    <div style="font-size: 2em; font-weight: 800; color: #0c4a6e;">{total_customers}</div>
                                </div>
                                <div style="flex: 1; background: #fef2f2; border-left: 6px solid #ef4444; padding: 20px; border-radius: 8px;">
                                    <div style="font-size: 0.8em; font-weight: 700; color: #b91c1c; text-transform: uppercase;">"Critical Issues"</div>
                                    <div style="font-size: 2em; font-weight: 800; color: #7f1d1d;">{critical}</div>
                                </div>
                            </div>
                        }.into_view()
                    } else { view! {}.into_view() })}
                </Transition>

                // SEARCH BAR
                <div style="margin-bottom: 25px;">
                    <input type="text" placeholder="Quick Filter (Customer, Group, Instance, or Status)..." 
                        style="width: 100%; padding: 14px; border: 1px solid #e2e8f0; border-radius: 10px; font-size: 1em; outline: none; transition: border-color 0.2s;"
                        on:input=move |ev| set_search_query.set(event_target_value(&ev)) />
                </div>

                // NAVIGATION BREADCRUMB
                {move || if let Some(cust) = selected_customer.get() {
                    view! {
                        <div style="display: flex; align-items: center; gap: 10px; margin-bottom: 20px;">
                            <button on:click=move |_| set_selected_customer.set(None) 
                                style="background: #64748b; color: white; border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 600;">"← Back to Summary"</button>
                            <h2 style="margin: 0; color: #334155;">"Tenancy: " {cust}</h2>
                        </div>
                    }.into_view()
                } else { view! { <h3 style="color: #475569; margin-bottom: 15px;">"Tenancy Health Overview"</h3> }.into_view() }}

                // MAIN CONTENT AREA
                <Transition fallback=|| view! { <div style="text-align: center; padding: 50px; color: #64748b;">"Syncing with Object Storage..."</div> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(insts) => {
                            let q = search_query.get().to_lowercase();
                            if let Some(cust) = selected_customer.get() {
                                render_detail_table(insts, cust, q)
                            } else {
                                render_summary_grid(insts, q, set_selected_customer)
                            }
                        },
                        Err(e) => view! { <div style="color: #ef4444; background: #fef2f2; padding: 20px; border-radius: 8px; border: 1px solid #fee2e2;">"Sync Error: " {e}</div> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

// 2. SUMMARY GRID VIEW
fn render_summary_grid(insts: Vec<HealthInstance>, query: String, set_selected: WriteSignal<Option<String>>) -> View {
    let mut stats: HashMap<String, (i32, i32)> = HashMap::new();
    
    for i in insts {
        let name = i.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        let entry = stats.entry(name).or_insert((0, 0));
        
        let h = i.health_status.as_deref().unwrap_or("").to_uppercase();
        let s = i.instance_status.as_deref().unwrap_or("").to_uppercase();
        
        if h == "FAILED" || s == "STOPPED" || s == "FAILED" { entry.1 += 1; }
        else { entry.0 += 1; }
    }

    let filtered: Vec<_> = stats.into_iter()
        .filter(|(name, _)| name.to_lowercase().contains(&query))
        .collect();

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 15px;">
            {filtered.into_iter().map(|(name, (ok, err))| {
                let n = name.clone();
                let border_color = if err > 0 { "#ef4444" } else { "#22c55e" };
                view! {
                    <div on:click=move |_| set_selected.set(Some(n.clone()))
                         style=format!("padding: 20px; border: 1px solid #e2e8f0; border-radius: 12px; cursor: pointer; transition: transform 0.1s; background: white; border-top: 5px solid {};", border_color)>
                        <h4 style="margin: 0 0 15px 0; color: #0f172a; font-size: 1.1em;">{name}</h4>
                        <div style="display: flex; justify-content: space-between; font-weight: 700; font-size: 0.9em;">
                            <span style="color: #16a34a;">"✔ " {ok} " Healthy"</span>
                            <span style="color: #dc2626;">"✘ " {err} " Critical"</span>
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

// 3. DETAIL TABLE VIEW (Fixes Server Group & Raw JSON)
fn render_detail_table(insts: Vec<HealthInstance>, customer: String, query: String) -> View {
    let mut filtered: Vec<_> = insts.into_iter()
        .filter(|i| i.customer_name.as_ref() == Some(&customer))
        .filter(|i| {
            let searchable = format!("{} {} {} {}", 
                i.server_group.as_deref().unwrap_or(""),
                i.instance_name.as_deref().unwrap_or(""),
                i.health_status.as_deref().unwrap_or(""),
                i.instance_status.as_deref().unwrap_or("")
            ).to_lowercase();
            searchable.contains(&query)
        })
        .collect();

    // Sort by Server Group then Instance Name
    filtered.sort_by(|a, b| a.server_group.cmp(&b.server_group).then(a.instance_name.cmp(&b.instance_name)));

    view! {
        <div style="overflow-x: auto; border: 1px solid #e2e8f0; border-radius: 10px;">
            <table style="width: 100%; border-collapse: collapse; background: white; font-size: 0.9em;">
                <thead>
                    <tr style="background: #f1f5f9; color: #475569; text-align: left; border-bottom: 2px solid #e2e8f0;">
                        <th style="padding: 15px; width: 40%;">"Raw Metrics JSON (Reference)"</th>
                        <th style="padding: 15px;">"Server Group"</th>
                        <th style="padding: 15px;">"Instance"</th>
                        <th style="padding: 15px;">"Status"</th>
                        <th style="padding: 15px;">"Last Sync"</th>
                    </tr>
                </thead>
                <tbody>
                    {filtered.into_iter().map(|i| {
                        let h = i.health_status.as_deref().unwrap_or("").to_uppercase();
                        let s = i.instance_status.as_deref().unwrap_or("").to_uppercase();
                        
                        let (display_text, bg_color) = if h == "FAILED" || s == "FAILED" { ("CRITICAL", "#ef4444") }
                                                       else if s == "STOPPED" { ("STOPPED", "#94a3b8") }
                                                       else { ("HEALTHY", "#22c55e") };
                        
                        let raw_json = serde_json::to_string(&i).unwrap_or_default();
                        
                        view! {
                            <tr style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;">
                                <td style="padding: 12px; font-family: 'Cascadia Code', Consolas, monospace; font-size: 0.75em; color: #64748b; line-height: 1.4; white-space: pre-wrap; word-break: break-all;">
                                    {raw_json}
                                </td>
                                <td style="padding: 12px; font-weight: 700; color: #1e293b;">
                                    {i.server_group.unwrap_or_else(|| "N/A".into())}
                                </td>
                                <td style="padding: 12px; color: #334155;">
                                    {i.instance_name.unwrap_or_else(|| "Unknown".into())}
                                </td>
                                <td style="padding: 12px;">
                                    <span style=format!("padding: 4px 10px; border-radius: 6px; color: white; background: {}; font-weight: 800; font-size: 0.75em; display: inline-block;", bg_color)>
                                        {display_text}
                                    </span>
                                </td>
                                <td style="padding: 12px; font-family: monospace; color: #94a3b8; font-size: 0.8em;">
                                    {i.last_sync.unwrap_or_default()}
                                </td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }.into_view()
}

// 4. FETCH  the LOGIC (With Cache Busting)
async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?cache_bust={}", js_sys::Date::now());
    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    
    if !resp.ok() { return Err(format!("Server returned {}", resp.status())); }
    
    let text = resp.text().await.map_err(|e| e.to_string())?;
    
    // Attempt to parse as list directly (Common merge result)
    if let Ok(list) = serde_json::from_str::<Vec<HealthInstance>>(&text) {
        return Ok(list);
    }
    
    // Fallback: If your sync merges into a "instances" key
    #[derive(Deserialize)] struct Wrapper { instances: Vec<HealthInstance> }
    if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&text) {
        return Ok(wrapper.instances);
    }

    Err("Invalid JSON format from sync.".into())
}

fn main() { mount_to_body(|| view! { <App /> }) }