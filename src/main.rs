use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::HashMap;
use gloo_timers::callback::Interval;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)] pub customer_name: Option<String>,
    #[serde(default, alias = "servergroup")] pub server_group: Option<String>, 
    #[serde(default)] pub instance_name: Option<String>,
    #[serde(default)] pub health_status: Option<String>, 
    #[serde(default)] pub instance_status: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (selected_customer, set_selected_customer) = create_signal(None::<String>);
    let (refresh_count, set_refresh_count) = create_signal(0);
    
    let health_data = create_resource(move || refresh_count.get(), |_| async move { 
        fetch_health_data().await 
    });

    // 5-Minute Auto-Refresh
    core::mem::forget(Interval::new(300_000, move || {
        set_refresh_count.update(|n| *n += 1);
    }));

    view! {
        <div style="padding: 20px; font-family: sans-serif; background: #f1f5f9; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 10px 15px rgba(0,0,0,0.1);">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;">
                    <h2 style="color: #1e3a8a; margin: 0;">"JDE Global Health Monitor"</h2>
                    <span style="font-size: 0.8em; color: #64748b;">"Refreshes: " {move || refresh_count.get()}</span>
                </div>

                <Transition fallback=|| view! { <p>"Syncing..."</p> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(insts) => {
                            if let Some(cust) = selected_customer.get() {
                                render_grouped_detail_view(insts, cust, set_selected_customer)
                            } else {
                                render_summary_grid(insts, set_selected_customer)
                            }
                        },
                        Err(e) => view! { <div style="color: red;">"Error: " {e}</div> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

// Logic to determine if a server is actually failing
fn get_status_type(i: &HealthInstance) -> (&'static str, &'static str) {
    let h = i.health_status.as_deref().unwrap_or("").to_uppercase();
    let s = i.instance_status.as_deref().unwrap_or("").to_uppercase();
    let raw = serde_json::to_string(i).unwrap_or_default().to_uppercase();

    if h.contains("FAILED") || h.contains("ERROR") || s.contains("STOPPED") || raw.contains("CONNECTION_ERROR") {
        ("CRITICAL", "#ef4444") // Red
    } else if h.is_empty() || h == "NULL" {
        ("UNKNOWN", "#94a3b8") // Gray for null values
    } else {
        ("HEALTHY", "#22c55e") // Green
    }
}

fn render_summary_grid(insts: Vec<HealthInstance>, set_cust: WriteSignal<Option<String>>) -> View {
    let mut customers: HashMap<String, (i32, i32, i32)> = HashMap::new();
    for i in &insts {
        let name = i.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        let entry = customers.entry(name).or_insert((0, 0, 0));
        match get_status_type(i).0 {
            "CRITICAL" => entry.1 += 1,
            "UNKNOWN" => entry.2 += 1,
            _ => entry.0 += 1,
        }
    }

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 20px;">
            {customers.into_iter().map(|(name, (ok, err, unk))| {
                let n = name.clone();
                view! {
                    <div on:click=move |_| set_cust.set(Some(n.clone()))
                         style=format!("padding: 20px; border-radius: 12px; cursor: pointer; border: 1px solid #e2e8f0; border-top: 6px solid {};", if err > 0 { "#ef4444" } else { "#22c55e" })>
                        <h3 style="margin: 0;">{name}</h3>
                        <div style="margin-top: 10px; font-weight: bold; font-size: 0.9em;">
                            <span style="color: #16a34a;">{ok} " OK "</span>
                            <span style="color: #dc2626;">{err} " ERR "</span>
                            <span style="color: #64748b;">{unk} " UNK"</span>
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

fn render_grouped_detail_view(insts: Vec<HealthInstance>, customer: String, set_cust: WriteSignal<Option<String>>) -> View {
    let mut groups: HashMap<String, Vec<HealthInstance>> = HashMap::new();
    for i in insts.into_iter().filter(|i| i.customer_name.as_ref() == Some(&customer)) {
        // Use the explicit server_group value from your JSON
        let group_name = i.server_group.clone().unwrap_or_else(|| "General".into());
        groups.entry(group_name).or_default().push(i);
    }

    view! {
        <div>
            <button on:click=move |_| set_cust.set(None) style="margin-bottom: 20px;">"← Back"</button>
            {groups.into_iter().map(|(group_name, members)| {
                view! {
                    <div style="margin-bottom: 30px; border: 1px solid #e2e8f0; border-radius: 8px; background: white;">
                        <div style="background: #f8fafc; padding: 10px 15px; border-bottom: 2px solid #1e3a8a;">
                            <h4 style="margin: 0; color: #1e3a8a;">{group_name} " Group"</h4>
                        </div>
                        <table style="width: 100%; border-collapse: collapse;">
                            <thead>
                                <tr style="background: #f1f5f9; text-align: left; font-size: 0.8em;">
                                    <th style="padding: 10px;">"Instance"</th>
                                    <th style="padding: 10px;">"Status"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {members.into_iter().map(|m| {
                                    let (status_text, color) = get_status_type(&m);
                                    view! {
                                        <tr style="border-bottom: 1px solid #f1f5f9;">
                                            <td style="padding: 10px; font-weight: bold;">{m.instance_name.unwrap_or_default()}</td>
                                            <td style="padding: 10px;">
                                                <span style=format!("color: white; padding: 2px 8px; border-radius: 4px; font-size: 0.8em; font-weight: bold; background: {};", color)>
                                                    {status_text}
                                                </span>
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
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?t={}", js_sys::Date::now());
    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    resp.json::<Vec<HealthInstance>>().await.map_err(|e| e.to_string())
}

fn main() { mount_to_body(|| view! { <App /> }) }