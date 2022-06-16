use anyhow::Result;

use openxr::{
    Action, ActionSet, ActionState, ActiveActionSet, Binding, Instance, Path, Posef, Session,
    Space, Time, Vulkan, USER_HAND_LEFT, USER_HAND_RIGHT,
};

use super::Context;

pub struct State {
    pub hand_poses: [Posef; 2],
    pub trigger_clicks: [ActionState<bool>; 2],
    pub a_clicks: [ActionState<bool>; 2],
    pub b_clicks: [ActionState<bool>; 2],
    pub pad_or_stick_click: [ActionState<bool>; 2],
    pub pad_or_stick_position_x: [ActionState<f32>; 2],
    pub pad_or_stick_position_y: [ActionState<f32>; 2],
}

pub struct Actions {
    session: Session<Vulkan>,
    general_action_set: ActionSet,
    action_hand_pose: Action<Posef>,
    action_trigger_click: Action<bool>,
    action_a_click: Action<bool>,
    action_b_click: Action<bool>,
    action_pad_or_stick_click: Action<bool>,
    action_pad_or_stick_position_x: Action<f32>,
    action_pad_or_stick_position_y: Action<f32>,
    hand_pose_spaces: [Space; 2],
    subaction_paths: [Path; 2],
}

fn left_right_paths(instance: &Instance, suffix: &str) -> Result<[Path; 2]> {
    Ok([
        instance.string_to_path(&format!("{}{}", USER_HAND_LEFT, suffix))?,
        instance.string_to_path(&format!("{}{}", USER_HAND_RIGHT, suffix))?,
    ])
}

impl Actions {
    pub fn new(instance: &Instance, session: Session<Vulkan>) -> Result<Self> {
        let subaction_paths = left_right_paths(instance, "")?;

        // don't need any other set atm
        let general_action_set = instance.create_action_set("general_action_set", "General", 0)?;

        let action_hand_pose =
            general_action_set.create_action("hand_pose", "Hand Pose", &subaction_paths)?;
        let action_trigger_click =
            general_action_set.create_action("trigger_click", "Trigger Click", &subaction_paths)?;
        let action_a_click =
            general_action_set.create_action("a_click", "A Click", &subaction_paths)?;
        let action_b_click =
            general_action_set.create_action("b_click", "B Click", &subaction_paths)?;
        let action_pad_or_stick_click = general_action_set.create_action(
            "pad_or_stick_click",
            "Pad or Stick Click",
            &subaction_paths,
        )?;
        let action_pad_or_stick_position_x = general_action_set.create_action(
            "pad_or_stick_position_x",
            "Pad or Stick Position X",
            &subaction_paths,
        )?;
        let action_pad_or_stick_position_y = general_action_set.create_action(
            "pad_or_stick_position_y",
            "Pad or Stick Position Y",
            &subaction_paths,
        )?;

        let hand_pose_spaces = [
            action_hand_pose.create_space(session.clone(), subaction_paths[0], Posef::IDENTITY)?,
            action_hand_pose.create_space(session.clone(), subaction_paths[1], Posef::IDENTITY)?,
        ];

        let actions = Self {
            session,
            general_action_set,
            action_hand_pose,
            action_trigger_click,
            action_a_click,
            action_b_click,
            action_pad_or_stick_click,
            action_pad_or_stick_position_x,
            action_pad_or_stick_position_y,
            hand_pose_spaces,
            subaction_paths,
        };

        let suggest = |suggestion: Suggestion| {
            instance.suggest_interaction_profile_bindings(
                suggestion.platform_path,
                &[
                    Binding::new(&actions.action_hand_pose, suggestion.pose_paths[0]),
                    Binding::new(&actions.action_hand_pose, suggestion.pose_paths[1]),
                    Binding::new(
                        &actions.action_trigger_click,
                        suggestion.trigger_click_paths[0],
                    ),
                    Binding::new(
                        &actions.action_trigger_click,
                        suggestion.trigger_click_paths[1],
                    ),
                    Binding::new(&actions.action_a_click, suggestion.a_click_paths[0]),
                    Binding::new(&actions.action_a_click, suggestion.a_click_paths[1]),
                    Binding::new(&actions.action_b_click, suggestion.b_click_paths[0]),
                    Binding::new(&actions.action_b_click, suggestion.b_click_paths[1]),
                    Binding::new(
                        &actions.action_pad_or_stick_click,
                        suggestion.pad_or_stick_click_paths[0],
                    ),
                    Binding::new(
                        &actions.action_pad_or_stick_click,
                        suggestion.pad_or_stick_click_paths[1],
                    ),
                    Binding::new(
                        &actions.action_pad_or_stick_position_x,
                        suggestion.pad_or_stick_position_x_paths[0],
                    ),
                    Binding::new(
                        &actions.action_pad_or_stick_position_x,
                        suggestion.pad_or_stick_position_x_paths[1],
                    ),
                    Binding::new(
                        &actions.action_pad_or_stick_position_y,
                        suggestion.pad_or_stick_position_y_paths[0],
                    ),
                    Binding::new(
                        &actions.action_pad_or_stick_position_y,
                        suggestion.pad_or_stick_position_y_paths[1],
                    ),
                ],
            )
        };

        suggest(Suggestion::index(instance)?)?;
        suggest(Suggestion::vive(instance)?)?;

        actions
            .session
            .attach_action_sets(&[&actions.general_action_set])?;

        Ok(actions)
    }

    pub fn get_state(&self, reference: &Space, time: Time) -> Result<State> {
        let active_action_set = ActiveActionSet::new(&self.general_action_set);
        self.session.sync_actions(&[active_action_set])?;
        let hand_poses = [
            self.hand_pose_spaces[0].locate(reference, time)?.pose,
            self.hand_pose_spaces[1].locate(reference, time)?.pose,
        ];
        let trigger_clicks = [
            self.action_trigger_click
                .state(&self.session, self.subaction_paths[0])?,
            self.action_trigger_click
                .state(&self.session, self.subaction_paths[1])?,
        ];
        let a_clicks = [
            self.action_a_click
                .state(&self.session, self.subaction_paths[0])?,
            self.action_a_click
                .state(&self.session, self.subaction_paths[1])?,
        ];
        let b_clicks = [
            self.action_b_click
                .state(&self.session, self.subaction_paths[0])?,
            self.action_b_click
                .state(&self.session, self.subaction_paths[1])?,
        ];
        let pad_or_stick_click = [
            self.action_pad_or_stick_click
                .state(&self.session, self.subaction_paths[0])?,
            self.action_pad_or_stick_click
                .state(&self.session, self.subaction_paths[1])?,
        ];
        let pad_or_stick_position_x = [
            self.action_pad_or_stick_position_x
                .state(&self.session, self.subaction_paths[0])?,
            self.action_pad_or_stick_position_x
                .state(&self.session, self.subaction_paths[1])?,
        ];
        let pad_or_stick_position_y = [
            self.action_pad_or_stick_position_y
                .state(&self.session, self.subaction_paths[0])?,
            self.action_pad_or_stick_position_y
                .state(&self.session, self.subaction_paths[1])?,
        ];

        Ok(State {
            hand_poses,
            trigger_clicks,
            a_clicks,
            b_clicks,
            pad_or_stick_click,
            pad_or_stick_position_x,
            pad_or_stick_position_y,
        })
    }
}

struct Suggestion {
    platform_path: Path,
    pose_paths: [Path; 2],
    trigger_click_paths: [Path; 2],
    a_click_paths: [Path; 2],
    b_click_paths: [Path; 2],
    pad_or_stick_click_paths: [Path; 2],
    pad_or_stick_position_x_paths: [Path; 2],
    pad_or_stick_position_y_paths: [Path; 2],
}

impl Suggestion {
    fn index(instance: &Instance) -> Result<Self> {
        // https://www.khronos.org/registry/OpenXR/specs/1.0/html/xrspec.html#_valve_index_controller_profile
        Ok(Self {
            platform_path: instance
                .string_to_path("/interaction_profiles/valve/index_controller")?,
            pose_paths: left_right_paths(instance, "/input/grip/pose")?,
            trigger_click_paths: left_right_paths(instance, "/input/trigger/click")?,
            a_click_paths: left_right_paths(instance, "/input/a/click")?,
            b_click_paths: left_right_paths(instance, "/input/b/click")?,
            pad_or_stick_click_paths: left_right_paths(instance, "/input/thumbstick/click")?,
            pad_or_stick_position_x_paths: left_right_paths(instance, "/input/thumbstick/x")?,
            pad_or_stick_position_y_paths: left_right_paths(instance, "/input/thumbstick/y")?,
        })
    }

    fn vive(instance: &Instance) -> Result<Self> {
        // https://www.khronos.org/registry/OpenXR/specs/1.0/html/xrspec.html#_htc_vive_controller_profile
        // TODO: Not sure what the best mapping for A/B buttons is.
        Ok(Self {
            platform_path: instance.string_to_path("/interaction_profiles/htc/vive_controller")?,
            pose_paths: left_right_paths(instance, "/input/grip/pose")?,
            trigger_click_paths: left_right_paths(instance, "/input/trigger/click")?,
            a_click_paths: left_right_paths(instance, "/input/squeeze/click")?,
            b_click_paths: left_right_paths(instance, "/input/trackpad/click")?, // same as trackpad ? :P
            pad_or_stick_click_paths: left_right_paths(instance, "/input/trackpad/click")?,
            pad_or_stick_position_x_paths: left_right_paths(instance, "/input/trackpad/x")?,
            pad_or_stick_position_y_paths: left_right_paths(instance, "/input/trackpad/y")?,
        })
    }
}
