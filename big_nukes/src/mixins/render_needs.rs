fn render_needs(needs: &mut RenderNeeds) {
    // Big Nukes: the clouds' pass marches each ray up to the scene's depth.
    if ::mod_big_nukes::sky::needs_depth() {
        needs.world_depth = true;
    }
}
