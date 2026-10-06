//! Engine-owned retained scene representation.

#[derive(Debug, Clone, PartialEq)]
pub enum SceneCommand {
    Text {
        x: f32,
        y: f32,
        size_px: f32,
        text: String,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub commands: Vec<SceneCommand>,
    pub epoch: u64,
}

impl Scene {
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

pub trait SceneRenderer {
    type Artifact;

    fn name(&self) -> &'static str;
    fn render(&self, scene: &Scene, width: u32, height: u32) -> Self::Artifact;
}

#[derive(Debug, Default)]
pub struct DeterministicTextRenderer;

impl SceneRenderer for DeterministicTextRenderer {
    type Artifact = String;

    fn name(&self) -> &'static str {
        "awef-native-text-v0"
    }

    fn render(&self, scene: &Scene, width: u32, height: u32) -> Self::Artifact {
        let mut output = format!("AWEF-SCENE {width}x{height} epoch={}\n", scene.epoch);
        for command in &scene.commands {
            output.push_str(&format!("{command:?}\n"));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_is_deterministic_for_same_scene() {
        let scene = Scene {
            commands: vec![SceneCommand::Text {
                x: 1.0,
                y: 2.0,
                size_px: 16.0,
                text: "hello".into(),
            }],
            epoch: 7,
        };
        let renderer = DeterministicTextRenderer;
        assert_eq!(
            renderer.render(&scene, 320, 200),
            renderer.render(&scene, 320, 200)
        );
    }
}
