mod control;
mod export;
mod governance;
mod registry;
mod relations;
mod types;
mod verification;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            registry::inspect_inventory,
            registry::load_inventory,
            registry::refresh_inventory,
            relations::resolve_agent_skills,
            export::export_assets,
            control::load_control_center,
            control::save_model_profile,
            control::delete_model_profile,
            control::clear_model_credential,
            control::save_custom_binding,
            control::delete_custom_binding,
            control::test_model_profile,
            control::list_model_options,
            control::apply_tool_binding,
            control::restore_tool_binding,
            governance::load_asset_governance,
            governance::select_asset_variant,
            governance::confirm_asset_license,
            governance::confirm_asset_verification,
            governance::create_optimization_draft,
            governance::apply_optimization_draft,
            verification::run_asset_audit
        ])
        .run(tauri::generate_context!())
        .expect("灵栈启动失败");
}
