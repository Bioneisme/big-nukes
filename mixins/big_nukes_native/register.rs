fn big_nukes_register(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    crate::game::big_nukes_glue::commands(commands);
}
