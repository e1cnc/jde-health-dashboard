use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::{HashMap, HashSet};

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
    let health_data = create_resource(|| (), |_| async move { fetch_health_data().await });

    view! {
        <div style="padding: 20px; font-family: sans-serif; background: #f8fafc; min-height: 100vh;">
            <div style="max-width: 1400px; margin: auto; background: white; padding: 20px; border-radius: 12px; box-shadow: 0 4px 10px rgba(0,0,0,0.1);">
                
                <h2 style="color: #004488; border-bottom: 2px solid #004488; padding-bottom: 10px;">"JDE Global Health Monitor"</h2>

                <input type="text" placeholder="Filter by Customer or Group..." 
                    style="width: 100%; padding: 12px; margin: 20px 0; border: 1px solid #ddd; border-radius: 8px;"
                    on:input=move |ev| set_search_query.set(event_target_value(&ev)) />

                <Transition fallback=|| view! { <p>"Syncing..."</p> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(insts) => {
                            if let Some(cust) = selected_customer.get() {
                                render_detail_view(insts, cust, set_selected_customer)
                            } else {
                                render_summary_view(insts, search_query.get(), set_selected_customer)
                            }
                        },
                        Err(e) => view! { <div style="color: red;">"Sync Error: " {e}</div> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

// THE FIX: Deep check for ANY failure in the JSON string
fn is_crit(i: &HealthInstance) -> bool {
    let raw = serde_json::to_string(i).unwrap_or_default().to_uppercase();
    let h = i.health_status.as_deref().unwrap_or("").to_uppercase();
    let s = i.instance_status.as_deref().unwrap_or("").to_uppercase();

    // Catch explicit status failures
    if h.contains("FAILED") || s.contains("FAILED") || s.contains("STOPPED") { return true; }
    // Catch Connection Errors or internal JDE failures found in your images
    if raw.contains("CONNECTION_ERROR") || raw.contains("ERROR") || raw.contains("FAILED") { return true; }
    
    false
}

fn render_summary_view(insts: Vec<HealthInstance>, query: String, set_cust: WriteSignal<Option<String>>) -> View {
    let mut customers: HashMap<String, (i32, i32)> = HashMap::new();
    for i in insts {
        let name = i.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        if name.to_lowercase().contains(&query.to_lowercase()) {
            let entry = customers.entry(name).or_insert((0, 0));
            if is_crit(&i) { entry.1 += 1; } else { entry.0 += 1; }
        }
    }

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 15px;">
            {customers.into_iter().map(|(name, (ok, err))| {
                let n = name.clone();
                view! {
                    <div on:click=move |_| set_cust.set(Some(n.clone()))
                         style=format!("padding: 15px; border-radius: 8px; cursor: pointer; border: 1px solid #ddd; border-left: 6px solid {};", if err > 0 { "#ef4444" } else { "#22c55e" })>
                        <h4 style="margin: 0;">{name}</h4>
                        <div style="font-size: 0.9em; margin-top: 10px; font-weight: bold;">
                            <span style="color: #22c55e;">"✔ " {ok} " OK "</span>
                            <span style="color: #ef4444;">"✘ " {err} " ERR"</span>
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

fn render_detail_view(insts: Vec<HealthInstance>, customer: String, set_cust: WriteSignal<Option<String>>) -> View {
    let mut groups: HashMap<String, Vec<HealthInstance>> = HashMap::new();
    for i in insts.into_iter().filter(|i| i.customer_name.as_ref() == Some(&customer)) {
        let g = i.server_group.clone().unwrap_or_else(|| "Other".into());
        groups.entry(g).or_default().push(i);
    }

    view! {
        <div>
            <button on:click=move |_| set_cust.set(None) style="margin-bottom: 20px; padding: 8px 16px;">"← Back"</button>
            {groups.into_iter().map(|(group_name, members)| {
                let mut ok = 0; let mut err = 0;
                for m in &members { if is_crit(m) { err += 1; } else { ok += 1; } }
                view! {
                    <div style="margin-bottom: 30px; border: 1px solid #eee; border-radius: 8px; overflow: hidden;">
                        <div style="background: #f1f5f9; padding: 10px 15px; display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #004488;">
                            <h3 style="margin: 0; color: #004488;">{group_name}</h3>
                            <div style="font-weight: bold; font-size: 0.85em;">
                                <span style="color: #16a34a;">"Healthy: " {ok}</span>" | "
                                <span style="color: #dc2626;">"Critical: " {err}</span>
                            </div>
                        </div>
                        <table style="width: 100%; border-collapse: collapse; font-size: 0.85em;">
                            <thead>
                                <tr style="background: #fafafa; text-align: left; border-bottom: 1px solid #ddd;">
                                    <th style="padding: 10px;">"Instance"</th>
                                    <th style="padding: 10px;">"Status"</th>
                                    <th style="padding: 10px;">"Details (Raw JSON)"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {members.into_iter().map(|m| {
                                    let crit = is_crit(&m);
                                    let raw = serde_json::to_string(&m).unwrap_or_default();
                                    view! {
                                        <tr style="border-bottom: 1px solid #eee;">
                                            <td style="padding: 10px; font-weight: bold;">{m.instance_name.unwrap_or_default()}</td>
                                            <td style="padding: 10px;">
                                                <span style=format!("padding: 3px 8px; border-radius: 4px; color: white; font-weight: bold; background: {};", if crit { "#ef4444" } else { "#22c55e" })>
                                                    {if crit { "CRITICAL" } else { "HEALTHY" }}
                                                </span>
                                            </td>
                                            <td style="padding: 10px; font-family: monospace; font-size: 0.75em; color: #666; word-break: break-all;">{raw}</td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?t={}", js_sys::Date::now());
    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    serde_json::from_str::<Vec<HealthInstance>>(&text).map_err(|e| e.to_string())
}

fn main() { mount_to_body(|| view! { <App /> }) }