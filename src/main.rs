use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub instance_status: Option<String>,
    pub health_status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvStatus {
    pub customer: String,
    pub env_name: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
}

#[derive(Deserialize, Debug)]
pub struct OCIObject { pub name: String }

#[derive(Deserialize, Debug)]
pub struct OCIListResponse { pub objects: Vec<OCIObject> }

#[derive(Clone, Copy, PartialEq)]
enum Filter { All, Failed, Healthy }

const BASE_URL: &str = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

async fn fetch_jde_health_data() -> Result<Vec<EnvStatus>, String> {
    let mut results = Vec::new();

    // 1. Discover all files in the bucket
    let list_url = format!("{}?format=json", BASE_URL);
    let resp = Request::get(&list_url).send().await
        .map_err(|e| format!("List API Failed: {}", e))?;
    
    let list_data: OCIListResponse = resp.json().await
        .map_err(|_| "Failed to parse OCI object list")?;

    // 2. Filter for only the 'latest' files we saw in the debugger
    let target_files: Vec<String> = list_data.objects.into_iter()
        .map(|obj| obj.name)
        .filter(|name| name.to_lowercase().ends_with("_latest.json"))
        .collect();

    // 3. Read each discovered file one by one
    for filename in target_files {
        let file_url = format!("{}/{}", BASE_URL, filename);
        
        if let Ok(file_resp) = Request::get(&file_url).send().await {
            if let Ok(instances) = file_resp.json::<Vec<HealthInstance>>().await {
                // Split filename (e.g. "LSJJOLDTR_PY_latest.json") to get labels
                let parts: Vec<&str> = filename.split('_').collect();
                let cust = parts.get(0).unwrap_or(&"UNKNOWN").to_string();
                let env = parts.get(1).unwrap_or(&"UNKNOWN").to_uppercase();

                let (mut ok, mut err) = (0, 0);
                for inst in &instances {
                    let s = inst.instance_status.as_deref().unwrap_or("").to_uppercase();
                    let h = inst.health_status.as_deref().unwrap_or("").to_lowercase();
                    if s == "RUNNING" && h == "passed" { ok += 1; } else { err += 1; }
                }

                results.push(EnvStatus {
                    customer: cust,
                    env_name: env,
                    total: instances.len(),
                    ok,
                    err,
                });
            }
        }
    }

    if results.is_empty() { return Err("No health data found in discovered files.".to_string()); }
    results.sort_by(|a, b| a.customer.cmp(&b.customer));
    Ok(results)
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(Filter::All);
    let health_resource = create_resource(|| (), |_| async move { fetch_jde_health_data().await });

    view! {
        <div style="padding: 25px; background: #f8fafc; min-height: 100vh; font-family: sans-serif;">
            <div style="max-width: 1200px; margin: auto;">
                
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px;">
                    <div>
                        <h2 style="margin: 0; color: #0f172a; font-weight: 800;">"JDE GLOBAL MONITOR"</h2>
                        <p style="margin: 0; font-size: 0.75rem; color: #64748b; font-weight: 700;">"AUTO-DISCOVERY ACTIVE"</p>
                    </div>
                    
                    <div style="display: flex; gap: 5px; background: #f1f5f9; padding: 4px; border-radius: 8px;">
                        <button on:click=move |_| set_filter.set(Filter::All) style=move || format!("border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 700; background: {}; color: {};", if filter.get() == Filter::All { "#1e293b" } else { "transparent" }, if filter.get() == Filter::All { "white" } else { "#64748b" })>"ALL"</button>
                        <button on:click=move |_| set_filter.set(Filter::Failed) style=move || format!("border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 700; background: {}; color: {};", if filter.get() == Filter::Failed { "#ef4444" } else { "transparent" }, if filter.get() == Filter::Failed { "white" } else { "#64748b" })>"FAILED"</button>
                        <button on:click=move |_| set_filter.set(Filter::Healthy) style=move || format!("border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 700; background: {}; color: {};", if filter.get() == Filter::Healthy { "#10b981" } else { "transparent" }, if filter.get() == Filter::Healthy { "white" } else { "#64748b" })>"HEALTHY"</button>
                    </div>
                </div>

                <Transition fallback=|| view! { <p>"Processing Discovered Files..."</p> }>
                    {move || health_resource.get().map(|res| match res {
                        Err(e) => view! { <div style="color: #ef4444;">{e}</div> }.into_view(),
                        Ok(items) => {
                            let total_ok: usize = items.iter().map(|i| i.ok).sum();
                            let total_inst: usize = items.iter().map(|i| i.total).sum();
                            let health_pct = if total_inst > 0 { (total_ok as f32 / total_inst as f32) * 100.0 } else { 0.0 };

                            let filtered: Vec<_> = items.into_iter().filter(|item| {
                                match filter.get() {
                                    Filter::All => true,
                                    Filter::Failed => item.err > 0,
                                    Filter::Healthy => item.err == 0,
                                }
                            }).collect();

                            view! {
                                <div style="background: white; border-radius: 12px; padding: 20px; margin-bottom: 25px; box-shadow: 0 1px 3px rgba(0,0,0,0.1);">
                                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;">
                                        <span style="font-weight: 700; color: #1e293b;">"SYSTEM WIDE HEALTH"</span>
                                        <span style=format!("font-weight: 800; color: {};", if health_pct > 90.0 { "#10b981" } else { "#ef4444" })>{format!("{:.1}%", health_pct)}</span>
                                    </div>
                                    <div style="background: #f1f5f9; height: 8px; border-radius: 4px; overflow: hidden;">
                                        <div style=format!("background: #10b981; height: 100%; width: {}%; transition: width 0.4s;", health_pct)></div>
                                    </div>
                                    <div style="margin-top: 10px; font-size: 0.7rem; font-weight: 700; color: #94a3b8;">{format!("{} OK / {} TOTAL", total_ok, total_inst)}</div>
                                </div>

                                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 20px;">
                                    {filtered.into_iter().map(|item| {
                                        let is_healthy = item.err == 0;
                                        view! {
                                            <div style=format!("background: white; border-radius: 12px; padding: 20px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1); border-top: 4px solid {};", if is_healthy { "#10b981" } else { "#ef4444" })>
                                                <div style="color: #94a3b8; font-size: 0.7rem; font-weight: 800; text-transform: uppercase;">{item.customer}</div>
                                                <div style="color: #1e293b; font-size: 1.4rem; font-weight: 900; margin-bottom: 15px;">{item.env_name}</div>
                                                <div style="display: flex; justify-content: space-between; align-items: center; border-top: 1px solid #f1f5f9; padding-top: 15px;">
                                                    <div>
                                                        <div style=format!("font-weight: 800; font-size: 0.8rem; color: {};", if is_healthy { "#059669" } else { "#dc2626" })>{if is_healthy { "HEALTHY" } else { "ERROR" }}</div>
                                                        <div style="font-size: 0.75rem; color: #64748b;">{format!("{}/{} OK", item.ok, item.total)}</div>
                                                    </div>
                                                    <div style=format!("font-size: 1.6rem; font-weight: 900; color: {};", if is_healthy { "#10b981" } else { "#ef4444" })>{format!("{:.0}%", (item.ok as f32 / item.total as f32) * 100.0)}</div>
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_view()
                        }
                    })}
                </Transition>
            </div>
        </div>
    }
}

fn main() { mount_to_body(|| view! { <App /> }) }