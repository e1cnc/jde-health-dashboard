use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)] pub customer_name: Option<String>,
    #[serde(default, alias = "serverGroup")] pub server_group: Option<String>, 
    #[serde(default, alias = "instanceName")] pub instance_name: Option<String>,
    #[serde(default, alias = "healthStatus")] pub health_status: Option<String>, 
    #[serde(default, alias = "instanceStatus")] pub instance_status: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (selected_customer, set_selected_customer) = create_signal(None::<String>);
    let health_data = create_resource(|| (), |_| async move { fetch_health_data().await });

    view! {
        <div style="padding: 20px; font-family: sans-serif; background: #f1f5f9; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 10px 15px -3px rgba(0,0,0,0.1);">
                <h2 style="color: #1e3a8a; border-bottom: 3px solid #1e3a8a; padding-bottom: 10px; margin-bottom: 30px;">"JDE Global Health Monitor"</h2>

                <Transition fallback=|| view! { <p>"Syncing Data..."</p> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(insts) => {
                            if let Some(cust) = selected_customer.get() {
                                render_grouped_detail_view(insts, cust, set_selected_customer)
                            } else {
                                render_summary_grid(insts, set_selected_customer)
                            }
                        },
                        Err(e) => view! { <div style="color: #b91c1c; padding: 20px; background: #fef2f2;">"Connection Error: " {e}</div> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

// DETERMINES CRITICAL STATUS
// If status is null, empty, or contains ERROR/FAILED/STOPPED, it is CRITICAL.
fn check_is_critical(i: &HealthInstance) -> bool {
    let h = i.health_status.as_deref().unwrap_or("NULL").to_uppercase();
    let s = i.instance_status.as_deref().unwrap_or("NULL").to_uppercase();
    let raw = serde_json::to_string(i).unwrap_or_default().to_uppercase();

    if h == "NULL" || s == "NULL" || h.is_empty() || s.is_empty() { return true; }
    if h.contains("FAILED") || h.contains("ERROR") || s.contains("STOPPED") || s.contains("FAILED") { return true; }
    if raw.contains("CONNECTION_ERROR") || raw.contains("JWT") { return true; }
    
    false
}

// HELPER: Extracts "DV", "PY", "PD" from "DV_JAS1"
fn get_group(i: &HealthInstance) -> String {
    if let Some(ref g) = i.server_group { if !g.is_empty() { return g.clone(); } }
    if let Some(ref name) = i.instance_name {
        if let Some(prefix) = name.split('_').next() {
            return prefix.to_uppercase();
        }
    }
    "OTHER".into()
}

fn render_summary_grid(insts: Vec<HealthInstance>, set_cust: WriteSignal<Option<String>>) -> View {
    let mut customers: HashMap<String, (i32, i32)> = HashMap::new();
    for i in &insts {
        let name = i.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        let entry = customers.entry(name).or_insert((0, 0));
        if check_is_critical(i) { entry.1 += 1; } else { entry.0 += 1; }
    }

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 20px;">
            {customers.into_iter().map(|(name, (ok, err))| {
                let n = name.clone();
                view! {
                    <div on:click=move |_| set_cust.set(Some(n.clone()))
                         style=format!("padding: 20px; border-radius: 12px; cursor: pointer; background: white; border: 1px solid #e2e8f0; border-top: 6px solid {};", if err > 0 { "#ef4444" } else { "#22c55e" })>
                        <h3 style="margin: 0 0 10px 0; color: #1e293b;">{name}</h3>
                        <div style="display: flex; gap: 15px; font-weight: bold; font-size: 0.9em;">
                            <span style="color: #16a34a;">"✔ " {ok} " Healthy"</span>
                            <span style="color: #dc2626;">"✘ " {err} " Critical"</span>
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

fn render_grouped_detail_view(insts: Vec<HealthInstance>, customer: String, set_cust: WriteSignal<Option<String>>) -> View {
    // Grouping logic based on prefix (DV, PY, PD)
    let mut groups: HashMap<String, Vec<HealthInstance>> = HashMap::new();
    for i in insts.into_iter().filter(|i| i.customer_name.as_ref() == Some(&customer)) {
        groups.entry(get_group(&i)).or_default().push(i);
    }

    view! {
        <div>
            <button on:click=move |_| set_cust.set(None) 
                style="background: #475569; color: white; border: none; padding: 10px 20px; border-radius: 6px; cursor: pointer; margin-bottom: 25px; font-weight: bold;">"← Global Overview"</button>
            
            <h2 style="color: #334155; margin-bottom: 20px;">"Tenancy: " {customer}</h2>

            {groups.into_iter().map(|(group_name, members)| {
                let mut ok = 0; let mut err = 0;
                for m in &members { if check_is_critical(m) { err += 1; } else { ok += 1; } }
                
                view! {
                    <div style="margin-bottom: 40px; border: 1px solid #e2e8f0; border-radius: 8px; overflow: hidden; background: white;">
                        <div style="background: #f8fafc; padding: 15px; display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #1e3a8a;">
                            <h3 style="margin: 0; color: #1e3a8a; font-size: 1.2em;">{group_name} " Environment Health"</h3>
                            <div style="font-weight: 800; font-size: 0.9em;">
                                <span style="color: #16a34a;">"OK: " {ok}</span>" | "
                                <span style="color: #dc2626;">"CRITICAL: " {err}</span>
                            </div>
                        </div>
                        <table style="width: 100%; border-collapse: collapse; text-align: left;">
                            <thead>
                                <tr style="background: #f1f5f9; font-size: 0.8em; text-transform: uppercase; color: #64748b;">
                                    <th style="padding: 12px;">"Instance Name"</th>
                                    <th style="padding: 12px;">"Managed Status"</th>
                                    <th style="padding: 12px;">"Raw JSON Data"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {members.into_iter().map(|m| {
                                    let is_crit = check_is_critical(&m);
                                    let inst_name = m.instance_name.clone().unwrap_or_else(|| "N/A".into());
                                    let raw = serde_json::to_string(&m).unwrap_or_default();
                                    view! {
                                        <tr style="border-bottom: 1px solid #f1f5f9;">
                                            <td style="padding: 12px; font-weight: bold; color: #1e293b;">{inst_name}</td>
                                            <td style="padding: 12px;">
                                                <span style=format!("padding: 4px 10px; border-radius: 4px; color: white; font-weight: 800; font-size: 0.75em; background: {};", if is_crit { "#ef4444" } else { "#22c55e" })>
                                                    {if is_crit { "CRITICAL" } else { "HEALTHY" }}
                                                </span>
                                            </td>
                                            <td style="padding: 12px; font-family: monospace; font-size: 0.7em; color: #94a3b8; max-width: 400px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                                                {raw}
                                            </td>
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
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?cb={}", js_sys::Date::now());
    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    serde_json::from_str::<Vec<HealthInstance>>(&text).map_err(|e| e.to_string())
}

fn main() { mount_to_body(|| view! { <App /> }) }