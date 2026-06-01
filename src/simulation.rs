use crate::render::SceneActivity;

#[derive(Clone, Debug)]
pub struct ActivitySmoother {
    // Raw metrics update less often than frames; this state bridges those
    // sample jumps so the rendered world changes gradually.
    current: SceneActivity,
    response: f32,
}

impl ActivitySmoother {
    pub fn new(response: f32) -> Self {
        Self {
            current: SceneActivity::default(),
            response: normalize_response(response),
        }
    }

    pub fn step_towards(&mut self, target: &SceneActivity) -> SceneActivity {
        self.current = blend_scene_activity(&self.current, target, self.response);
        self.current.clone()
    }
}

fn blend_scene_activity(
    current: &SceneActivity,
    target: &SceneActivity,
    response: f32,
) -> SceneActivity {
    let core_count = target.core_loads().len();
    let mut core_loads = Vec::with_capacity(core_count);
    for index in 0..core_count {
        core_loads.push(blend(
            current.core_loads().get(index).copied().unwrap_or(0.0),
            target.core_loads()[index],
            response,
        ));
    }

    SceneActivity::from_core_loads(core_loads)
        .with_memory_pressure(blend(
            current.memory_pressure(),
            target.memory_pressure(),
            response,
        ))
        .with_network_flow(
            blend(
                current.network_download(),
                target.network_download(),
                response,
            ),
            blend(current.network_upload(), target.network_upload(), response),
        )
        .with_disk_activity(
            blend(current.disk_read(), target.disk_read(), response),
            blend(current.disk_write(), target.disk_write(), response),
        )
}

fn blend(current: f32, target: f32, response: f32) -> f32 {
    current + ((target - current) * response)
}

fn normalize_response(response: f32) -> f32 {
    if response.is_finite() {
        response.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
