#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneActivity {
    core_loads: Vec<f32>,
    memory_pressure: f32,
    network_download: f32,
    network_upload: f32,
    disk_read: f32,
    disk_write: f32,
}

impl SceneActivity {
    pub fn from_core_loads(core_loads: Vec<f32>) -> Self {
        Self::default().with_core_loads(core_loads)
    }

    pub fn with_core_loads(mut self, core_loads: Vec<f32>) -> Self {
        let core_loads = core_loads
            .into_iter()
            .map(normalize_unit_interval)
            .collect();

        self.core_loads = core_loads;
        self
    }

    pub fn with_memory_pressure(mut self, memory_pressure: f32) -> Self {
        self.memory_pressure = normalize_unit_interval(memory_pressure);
        self
    }

    pub fn with_network_flow(mut self, download: f32, upload: f32) -> Self {
        self.network_download = normalize_unit_interval(download);
        self.network_upload = normalize_unit_interval(upload);
        self
    }

    pub fn with_disk_activity(mut self, read: f32, write: f32) -> Self {
        self.disk_read = normalize_unit_interval(read);
        self.disk_write = normalize_unit_interval(write);
        self
    }

    pub fn core_loads(&self) -> &[f32] {
        &self.core_loads
    }

    pub fn memory_pressure(&self) -> f32 {
        self.memory_pressure
    }

    pub fn network_download(&self) -> f32 {
        self.network_download
    }

    pub fn network_upload(&self) -> f32 {
        self.network_upload
    }

    pub fn disk_read(&self) -> f32 {
        self.disk_read
    }

    pub fn disk_write(&self) -> f32 {
        self.disk_write
    }
}

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

fn normalize_unit_interval(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
