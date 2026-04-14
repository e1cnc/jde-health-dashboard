use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub instance_status: Option<String>,
    pub health_status: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GroupStatus {
    pub name: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CustomerSummary {
    pub name: String,
    pub groups: BTreeMap<String, GroupStatus>,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter { All, Failed, Healthy }

async fn fetch_all_jde_data() -> Result<BTreeMap<String, CustomerSummary>, String> {
    let mut customers: BTreeMap<String, CustomerSummary> = BTreeMap::new();
    
    // Explicitly listing all environment targets for both customers
    let targets = vec![
       ("LSJJNEWTR", "DV", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJNEWTR_dv_latest.json"),
        ("LSJJNEWTR", "PY", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJNEWTR_py_latest.json"),
        ("LSJJOLDTR", "PS", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_ps_latest.json"),
        ("LSJJOLDTR", "PY", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_py_latest.json"),
    ];

    for (c_name, g_name, url) in targets {
        if let Ok(resp) = Request::get(url).send().await {
            if let Ok(instances) = resp.json::<Vec<HealthInstance>>().await {
                let cust = customers.entry(c_name.to_string()).or_insert(CustomerSummary {
                    name: c_name.to_string(),
                    groups: BTreeMap::new(),
                });

                let mut ok_count = 0;
                let mut err_count = 0;

                for inst in &instances {
                    let s = inst.instance_status.as_deref().unwrap_or("").to_uppercase();
                    let h = inst.health_status.as_deref().unwrap_or("").to_lowercase();
                    if s == "RUNNING" && h == "passed" { ok_count += 1; } else { err_count += 1; }
                }

                cust.groups.insert(g_name.to_string(), GroupStatus {
                    name: g_name.to_string(),
                    total: instances.len(),
                    ok: ok_count,
                    err: err_count,
                });
            }
        }
    }
    
    if customers.is_empty() { return Err("Unable to load health data".to_string()); }
    Ok(customers)
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(Filter::All);
    let health_resource = create_resource(|| (), |_| async move { fetch_all_jde_data().await });

    view! {
        <div style="padding: 30px; background: #f8fafc; min-height: 100vh; font-family: sans-serif;">
            <div style="max-width: 1200px; margin: auto;">
                
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px;">
                    <h1 style="margin: 0; color: #0f172a; font-weight: 900; font-size: 1.8em;">"JDE HEALTH DASHBOARD"</h1>
                    <div style="display: flex; background: white; padding: 4px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.05);">
                        <button on:click=move |_| set_filter.set(Filter::All) 
                                style=move || format!("border: none; padding: 8px 16px; border-radius: 6px; font-weight: 800; cursor: pointer; transition: 0.2s; background: {}; color: {};", if filter.get() == Filter::All { "#1e293b" } else { "transparent" }, if filter.get() == Filter::All { "white" } else { "#64748b" })>"ALL"</button>
                        <button on:click=move |_| set_filter.set(Filter::Failed) 
                                style=move || format!("border: none; padding: 8px 16px; border-radius: 6px; font-weight: 800; cursor: pointer; transition: 0.2s; background: {}; color: {};", if filter.get() == Filter::Failed { "#ef4444" } else { "transparent" }, if filter.get() == Filter::Failed { "white" } else { "#64748b" })>"FAILED"</button>
                        <button on:click=move |_| set_filter.set(Filter::Healthy) 
                                style=move || format!("border: none; padding: 8px 16px; border-radius: 6px; font-weight: 800; cursor: pointer; transition: 0.2s; background: {}; color: {};", if filter.get() == Filter::Healthy { "#10b981" } else { "transparent" }, if filter.get() == Filter::Healthy { "white" } else { "#64748b" })>"HEALTHY"</button>
                    </div>
                </div>

                <Transition fallback=|| view! { <p>"Syncing with OCI Storage..."</p> }>
                    {move || match health_resource.get() {
                        None => view! { <p>"Loading..."</p> }.into_view(),
                        Some(Err(e)) => view! { <p style="color: #ef4444;">{format!("Error: {}", e)}</p> }.into_view(),
                        Some(Ok(data)) => {
                            let total_ok: usize = data.values().flat_map(|c| c.groups.values().map(|g| g.ok)).sum();
                            let total_err: usize = data.values().flat_map(|c| c.groups.values().map(|g| g.err)).sum();
                            let ok_pct = if (total_ok + total_err) > 0 { (total_ok as f32 / (total_ok + total_err) as f32) * 100.0 } else { 0.0 };

                            view! {
                                <div style="background: white; border-radius: 12px; padding: 20px; margin-bottom: 30px; box-shadow: 0 4px 6px rgba(0,0,0,0.05);">
                                    <div style="display: flex; justify-content: space-between; margin-bottom: 10px; font-weight: 900;">
                                        <span>"SYSTEM WIDE HEALTH"</span>
                                        <span style=move || format!("color: {};", if ok_pct < 100.0 { "#ef4444" } else { "#10b981" })>{format!("{:.1}%", ok_pct)}</span>
                                    </div>
                                    <div style="width: 100%; height: 10px; background: #f1f5f9; border-radius: 5px; overflow: hidden;">
                                        <div style=move || format!("width: {}%; background: #10b981; height: 100%; transition: 1s;", ok_pct)></div>
                                    </div>
                                    <div style="margin-top: 10px; font-size: 0.8em; font-weight: 700; color: #64748b;">
                                        {format!("{} OK / {} FAILED", total_ok, total_err)}
                                    </div>
                                </div>

                                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(350px, 1fr)); gap: 20px;">
                                    {data.into_iter().filter(|(_, cust)| {
                                        match filter.get() {
                                            Filter::All => true,
                                            Filter::Failed => cust.groups.values().any(|g| g.err > 0),
                                            Filter::Healthy => cust.groups.values().all(|g| g.err == 0),
                                        }
                                    }).map(|(_, cust)| {
                                        let has_any_err = cust.groups.values().any(|g| g.err > 0);
                                        let border_color = if has_any_err { "#ef4444" } else { "#10b981" };
                                        view! {
                                            <div style=format!("background: white; border-radius: 12px; padding: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.05); border-left: 5px solid {};", border_color)>
                                                <h2 style="margin: 0 0 15px 0; font-weight: 900; color: #1e293b;">{cust.name}</h2>
                                                <div style="display: flex; flex-wrap: wrap; gap: 8px;">
                                                    {cust.groups.values().cloned().map(|g| {
                                                        let has_err = g.err > 0;
                                                        let bg = if has_err { "#fee2e2" } else { "#f0fdf4" };
                                                        let txt = if has_err { "#991b1b" } else { "#166534" };
                                                        view! {
                                                            <div style=format!("background: {}; color: {}; padding: 6px 12px; border-radius: 6px; font-weight: 800; font-size: 0.75em;", bg, txt)>
                                                                {format!("{}: {}/{}", g.name, g.ok, g.total)}
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_view()
                        }
                    }}
                </Transition>
            </div>
        </div>
    }
}

fn main() { mount_to_body(|| view! { <App /> }) }