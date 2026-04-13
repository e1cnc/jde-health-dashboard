use std::collections::{HashMap, HashSet};

fn render_summary_view(
    instances: Vec<HealthInstance>, 
    query: String, 
    set_selected: WriteSignal<Option<String>>,
    set_search: WriteSignal<String>
) -> View {
    // Map of Customer Name -> (Unique Instance Set, Has Critical Error)
    let mut customer_stats: HashMap<String, (HashSet<(String, String)>, bool)> = HashMap::new();
    
    for inst in instances {
        let name = inst.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        let host = inst.host_name.clone().unwrap_or_else(|| "UnknownHost".into());
        let group = inst.group.clone().unwrap_or_else(|| "UnknownGroup".into());
        let status = inst.status.as_deref().unwrap_or("UNKNOWN").to_uppercase();
        
        let is_critical = status == "STOPPED" || status == "FAILED";
        let entry = customer_stats.entry(name).or_insert((HashSet::new(), false));
        
        // Add unique tuple of (Host, Group) to the set
        entry.0.insert((host, group));
        if is_critical { entry.1 = true; }
    }

    let mut sorted_customers: Vec<_> = customer_stats.into_iter()
        .filter(|(name, _)| name.to_lowercase().contains(&query))
        .collect();
    sorted_customers.sort_by(|a, b| a.0.cmp(&b.0));

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 15px;">
            {sorted_customers.into_iter().map(|(name, (unique_set, has_error))| {
                let count = unique_set.len(); // This will now correctly show 5 instead of 64
                let display_name = name.clone();
                let bg_color = if has_error { "#fff5f5" } else { "#fafafa" };
                let border_color = if has_error { "#feb2b2" } else { "#ddd" };

                view! {
                    <div 
                        on:click=move |_| {
                            set_selected.set(Some(name.clone()));
                            set_search.set(String::new());
                        }
                        style=format!(
                            "padding: 20px; border: 2px solid {}; border-radius: 10px; cursor: pointer; background: {};",
                            border_color, bg_color
                        )
                    >
                        <h4 style="margin: 0; color: #004488;">{display_name}</h4>
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 10px;">
                            <span style="font-size: 0.85em; color: #666;">{count} " Unique Instances"</span>
                            {if has_error {
                                view! { <span style="font-size: 0.75em; font-weight: bold; color: #c53030;">"● CRITICAL"</span> }.into_view()
                            } else {
                                view! { <span style="font-size: 0.75em; font-weight: bold; color: #38a169;">"● Healthy"</span> }.into_view()
                            }}
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

fn render_detail_view(instances: Vec<HealthInstance>, customer: String, query: String) -> View {
    // Use  a HashMap to only keep the latest record for each unique (Host, Instance)
    let mut latest_instances: HashMap<(String, String), HealthInstance> = HashMap::new();

    for inst in instances.into_iter().filter(|i| i.customer_name.as_ref() == Some(&customer)) {
        let key = (
            inst.host_name.clone().unwrap_or_default(),
            inst.group.clone().unwrap_or_default()
        );
        // Assuming the JSON order or timestamp allows us to just take the "last" seen
        latest_instances.insert(key, inst);
    }

    let filtered: Vec<_> = latest_instances.into_values()
        .filter(|inst| inst.group.as_deref().unwrap_or("").to_lowercase().contains(&query))
        .collect();

    view! {
        <table style="width: 100%; border-collapse: collapse;">
            <thead>
                <tr style="background-color: #004488; color: white; text-align: left;">
                    <th style="padding: 12px;">"Host"</th>
                    <th style="padding: 12px;">"Instance"</th>
                    <th style="padding: 12px;">"Status"</th>
                    <th style="padding: 12px;">"Last Update"</th>
                </tr>
            </thead>
            <tbody>
                {filtered.into_iter().map(|inst| {
                    let status_str = inst.status.clone().unwrap_or_else(|| "UNKNOWN".into());
                    let is_ok = status_str == "RUNNING" || status_str == "Passed";
                    let (bg, fg) = if is_ok { ("#e6fffa", "#234e52") } else { ("#fff5f5", "#742a2a") };
                    view! {
                        <tr style="border-bottom: 1px solid #edf2f7;">
                            <td style="padding: 12px;">{inst.host_name.clone().unwrap_or_default()}</td>
                            <td style="padding: 12px;">{inst.group.clone().unwrap_or_default()}</td>
                            <td style="padding: 12px;">
                                <span style=format!("padding: 4px 10px; border-radius: 20px; font-weight: bold; background: {}; color: {}; border: 1px solid {};", bg, fg, fg)>
                                    {status_str}
                                </span>
                            </td>
                            <td style="padding: 12px; font-size: 0.8em; color: #666;">{inst.last_sync.clone().unwrap_or_default()}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }.into_view()
}