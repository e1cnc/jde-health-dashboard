use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::HashMap;
use gloo_timers::callback::Interval;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub customer_name: Option<String>,
    #[serde(alias = "group_name")] pub server_group: Option<String>, 
    pub instance_name: Option<String>,
    pub instance_status: Option<String>, 
    pub health_status: Option<String>,
    pub details: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (selected_customer, set_selected_customer) = create_signal(None::<String>);
    let (refresh_count, set_refresh_count) = create_signal(0);
    let health_data = create_resource(move || refresh_count.get(), |_| async move { fetch_data().await });

    core::mem::forget(Interval::new(300_000, move || set_refresh_count.update(|n| *n += 1)));

    view! {
        <div style="padding: 20px; font-family: sans-serif; background: #f1f5f9; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 10px 15px rgba(0,0,0,0.1);">
                <h2 style="color: #1e3a8a; border-bottom: 2px solid #1e3a8a; padding-bottom: 10px;">"JDE Global Health Monitor"</h2>
                <Transition fallback=|| view! { <p>"Syncing..."</p> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(insts) => {
                            if let Some(cust) = selected_customer.get() {
                                render_details(insts, cust, set_selected_customer)
                            } else {
                                render_summary(insts, set_selected_customer)
                            }
                        },
                        Err(e) => view! { <div style="color: red;">"Error: " {e}</div> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

fn get_status_style(i: &HealthInstance) -> (&'static str, &'static str) {
    let h = i.health_status.as_deref().unwrap_or("").to_uppercase();
    let s = i.instance_status.as_deref().unwrap_or("").to_uppercase();
    if h.contains("FAILED") || s != "RUNNING" { ("CRITICAL", "#ef4444") } 
    else { ("HEALTHY", "#22c55e") }
}

fn render_summary(insts: Vec<HealthInstance>, set_cust: WriteSignal<Option<String>>) -> View {
    let mut customers: HashMap<String, (i32, i32)> = HashMap::new();
    for i in &insts {
        let name = i.customer_name.clone().unwrap_or_default();
        let entry = customers.entry(name).or_insert((0, 0));
        if get_status_style(i).0 == "CRITICAL" { entry.1 += 1 } else { entry.0 += 1 }
    }
    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 20px;">
            {customers.into_iter().map(|(name, (ok, err))| {
                let n = name.clone();
                view! {
                    <div on:click=move |_| set_cust.set(Some(n.clone()))
                         style=format!("padding: 20px; border-radius: 12px; cursor: pointer; border: 1px solid #e2e8f0; border-top: 6px solid {};", if err > 0 { "#ef4444" } else { "#22c55e" })>
                        <h3 style="margin: 0;">{name}</h3>
                        <p><strong>{ok} " OK"</strong> " | " <strong style="color: red;">{err} " ERR"</strong></p>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

fn render_details(insts: Vec<HealthInstance>, customer: String, set_cust: WriteSignal<Option<String>>) -> View {
    let mut groups: HashMap<String, Vec<HealthInstance>> = HashMap::new();
    for i in insts.into_iter().filter(|i| i.customer_name.as_ref() == Some(&customer)) {
        groups.entry(i.server_group.clone().unwrap_or_else(|| "General".into())).or_default().push(i);
    }
    view! {
        <div>
            <button on:click=move |_| set_cust.set(None)>"← Back"</button>
            {groups.into_iter().map(|(g, members)| view! {
                <div style="margin-top: 20px; border: 1px solid #ddd; border-radius: 8px;">
                    <div style="background: #f8fafc; padding: 10px; font-weight: bold;">{g}</div>
                    <table style="width: 100%; border-collapse: collapse;">
                        {members.into_iter().map(|m| {
                            let (txt, color) = get_status_style(&m);
                            view! {
                                <tr style="border-top: 1px solid #eee;">
                                    <td style="padding: 10px;">{m.instance_name}</td>
                                    <td style=format!("color: {}; font-weight: bold;", color)>{txt}</td>
                                    <td style="font-size: 0.8em; color: #666;">{m.details}</td>
                                </tr>
                            }
                        }).collect_view()}
                    </table>
                </div>
            }).collect_view()}
        </div>
    }.into_view()
}

async fn fetch_data() -> Result<Vec<HealthInstance>, String> {
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?t={}", js_sys::Date::now());
    Request::get(&url).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())
}

fn main() { mount_to_body(|| view! { <App /> }) }