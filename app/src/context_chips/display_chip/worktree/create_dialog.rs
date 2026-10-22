use std::path::PathBuf;

use warpui::elements::{
    ChildView, Container, CrossAxisAlignment, Empty, Expanded, Flex, MainAxisSize, ParentElement,
    Text,
};
use warpui::platform::FilePickerConfiguration;
use warpui::ui_components::components::UiComponent;
use warpui::{
    AppContext, Element, Entity, FocusContext, SingletonEntity as _, TypedActionView, View,
    ViewContext, ViewHandle,
};

use super::create_dialog_components::{
    SUPPORTING_FONT_SIZE, action_button, field_label, render_editor, single_line_editor,
};
use super::create_operation::{CreateMode, CreateWorktreeRequest, create_worktree};
use crate::appearance::Appearance;
use crate::editor::{EditorView, Event as EditorEvent};
use crate::ui_components::dialog::{Dialog, dialog_styles};
use crate::view_components::action_button::{
    ActionButton, NakedTheme, PrimaryTheme, SecondaryTheme,
};
use crate::worktree_agents::WorktreeMenuEntry;

const DIALOG_WIDTH: f32 = 480.;

#[derive(Debug)]
pub(super) enum CreateWorktreeDialogEvent {
    Dismissed,
    Created(WorktreeMenuEntry),
}

#[derive(Clone, Debug)]
pub(super) enum CreateWorktreeDialogAction {
    Cancel,
    ChooseFolder,
    FolderChosen(PathBuf),
    Create,
    SetMode(CreateMode),
}

pub(super) struct CreateWorktreeDialog {
    visible: bool,
    source_path: PathBuf,
    common_dir: String,
    parent_directory: PathBuf,
    mode: CreateMode,
    error: Option<String>,
    creating: bool,
    name_editor: ViewHandle<EditorView>,
    branch_editor: ViewHandle<EditorView>,
    cancel_button: ViewHandle<ActionButton>,
    choose_folder_button: ViewHandle<ActionButton>,
    create_button: ViewHandle<ActionButton>,
    new_branch_button: ViewHandle<ActionButton>,
    existing_branch_button: ViewHandle<ActionButton>,
}

impl CreateWorktreeDialog {
    pub(super) fn new(ctx: &mut ViewContext<Self>) -> Self {
        let name_editor = single_line_editor("worktree-name", ctx);
        let branch_editor = single_line_editor("existing-branch", ctx);
        ctx.subscribe_to_view(&name_editor, Self::handle_name_event);
        ctx.subscribe_to_view(&branch_editor, Self::handle_branch_event);
        let cancel_button = action_button(
            "Cancel",
            NakedTheme,
            CreateWorktreeDialogAction::Cancel,
            ctx,
        );
        let choose_folder_button = action_button(
            "Choose folder...",
            NakedTheme,
            CreateWorktreeDialogAction::ChooseFolder,
            ctx,
        );
        let create_button = action_button(
            "Create worktree",
            PrimaryTheme,
            CreateWorktreeDialogAction::Create,
            ctx,
        );
        let new_branch_button = action_button(
            "New branch",
            SecondaryTheme,
            CreateWorktreeDialogAction::SetMode(CreateMode::NewBranch),
            ctx,
        );
        let existing_branch_button = action_button(
            "Existing branch",
            SecondaryTheme,
            CreateWorktreeDialogAction::SetMode(CreateMode::ExistingBranch),
            ctx,
        );
        create_button.update(ctx, |button, ctx| button.set_disabled(true, ctx));
        Self {
            visible: false,
            source_path: PathBuf::new(),
            common_dir: String::new(),
            parent_directory: PathBuf::new(),
            mode: CreateMode::NewBranch,
            error: None,
            creating: false,
            name_editor,
            branch_editor,
            cancel_button,
            choose_folder_button,
            create_button,
            new_branch_button,
            existing_branch_button,
        }
    }

    pub(super) fn show(&mut self, entries: Vec<WorktreeMenuEntry>, ctx: &mut ViewContext<Self>) {
        self.source_path.clear();
        self.common_dir.clear();
        self.parent_directory.clear();
        self.mode = CreateMode::NewBranch;
        self.error = None;
        self.creating = false;
        self.name_editor.update(ctx, |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
            editor.set_placeholder_text("worktree-name", ctx);
        });
        self.branch_editor.update(ctx, |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
            editor.set_placeholder_text("existing-branch", ctx);
        });
        self.visible = true;
        let Some(current) = entries
            .iter()
            .find(|entry| entry.is_current)
            .or_else(|| entries.first())
        else {
            self.error = Some("No local Git repository is available.".to_string());
            self.sync_controls(ctx);
            ctx.notify();
            return;
        };
        self.source_path = PathBuf::from(&current.path);
        self.common_dir = current.common_dir.clone();
        self.parent_directory = self
            .source_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.source_path.clone());
        self.sync_controls(ctx);
        ctx.focus(&self.name_editor);
        ctx.notify();
    }

    pub(super) fn hide(&mut self, ctx: &mut ViewContext<Self>) {
        self.visible = false;
        self.creating = false;
        self.error = None;
        ctx.notify();
    }

    fn handle_name_event(
        &mut self,
        _: ViewHandle<EditorView>,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        self.handle_editor_event(event, ctx);
    }

    fn handle_branch_event(
        &mut self,
        _: ViewHandle<EditorView>,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        self.handle_editor_event(event, ctx);
    }

    fn handle_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Edited(_) => {
                self.error = None;
                self.sync_controls(ctx);
            }
            EditorEvent::Enter => self.submit(ctx),
            EditorEvent::Escape => ctx.emit(CreateWorktreeDialogEvent::Dismissed),
            _ => {}
        }
        ctx.notify();
    }

    fn set_mode(&mut self, mode: CreateMode, ctx: &mut ViewContext<Self>) {
        self.mode = mode;
        self.error = None;
        self.sync_controls(ctx);
        match self.mode {
            CreateMode::NewBranch => ctx.focus(&self.name_editor),
            CreateMode::ExistingBranch => ctx.focus(&self.branch_editor),
        }
        ctx.notify();
    }

    fn choose_folder(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_file_picker(
            |result, ctx| {
                if let Ok(paths) = result
                    && let Some(path) = paths.first()
                {
                    ctx.dispatch_typed_action(&CreateWorktreeDialogAction::FolderChosen(
                        PathBuf::from(path),
                    ));
                }
            },
            FilePickerConfiguration::new().folders_only(),
        );
    }

    fn submit(&mut self, ctx: &mut ViewContext<Self>) {
        if self.creating {
            return;
        }
        let request = self.request(ctx);
        if let Err(error) = request.validate() {
            self.error = Some(error);
            self.sync_controls(ctx);
            ctx.notify();
            return;
        }
        self.creating = true;
        self.error = None;
        self.sync_controls(ctx);
        ctx.spawn(create_worktree(request), |view, result, ctx| {
            view.creating = false;
            match result {
                Ok(entry) => {
                    view.visible = false;
                    ctx.emit(CreateWorktreeDialogEvent::Created(entry));
                }
                Err(error) => view.error = Some(error),
            }
            view.sync_controls(ctx);
            ctx.notify();
        });
    }

    fn request(&self, app: &AppContext) -> CreateWorktreeRequest {
        let name = self
            .name_editor
            .as_ref(app)
            .buffer_text(app)
            .trim()
            .to_string();
        let branch = match self.mode {
            CreateMode::NewBranch => name.clone(),
            CreateMode::ExistingBranch => self
                .branch_editor
                .as_ref(app)
                .buffer_text(app)
                .trim()
                .to_string(),
        };
        CreateWorktreeRequest {
            source_path: self.source_path.clone(),
            common_dir: self.common_dir.clone(),
            destination: self.parent_directory.join(&name),
            worktree_name: name,
            mode: self.mode.clone(),
            branch,
        }
    }

    fn sync_controls(&self, ctx: &mut ViewContext<Self>) {
        let request = self.request(ctx);
        let disabled = self.creating || request.validate().is_err();
        self.create_button.update(ctx, |button, ctx| {
            button.set_disabled(disabled, ctx);
            button.set_label(
                if self.creating {
                    "Creating..."
                } else {
                    "Create worktree"
                },
                ctx,
            );
        });
        self.new_branch_button.update(ctx, |button, ctx| {
            button.set_active(self.mode == CreateMode::NewBranch, ctx)
        });
        self.existing_branch_button.update(ctx, |button, ctx| {
            button.set_active(self.mode == CreateMode::ExistingBranch, ctx)
        });
    }

    fn render_body(&self, app: &AppContext, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let name = self.name_editor.as_ref(app).buffer_text(app);
        let destination = self.parent_directory.join(name.trim());
        let mut body = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(field_label("Worktree name", appearance))
            .with_child(render_editor(&self.name_editor, appearance))
            .with_child(
                Container::new(
                    Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(ChildView::new(&self.new_branch_button).finish())
                        .with_child(
                            Container::new(ChildView::new(&self.existing_branch_button).finish())
                                .with_margin_left(6.)
                                .finish(),
                        )
                        .finish(),
                )
                .with_margin_top(12.)
                .finish(),
            );
        match self.mode {
            CreateMode::NewBranch => body.add_child(
                Container::new(
                    Text::new_inline(
                        "Creates a branch with the same name from the current HEAD.",
                        appearance.ui_font_family(),
                        SUPPORTING_FONT_SIZE,
                    )
                    .with_color(theme.sub_text_color(theme.surface_1()).into())
                    .finish(),
                )
                .with_margin_top(7.)
                .finish(),
            ),
            CreateMode::ExistingBranch => {
                body.add_child(
                    Container::new(field_label("Branch", appearance))
                        .with_margin_top(10.)
                        .finish(),
                );
                body.add_child(render_editor(&self.branch_editor, appearance));
            }
        }
        body.add_child(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(
                        Expanded::new(
                            1.,
                            Text::new_inline(
                                self.parent_directory.to_string_lossy().into_owned(),
                                appearance.ui_font_family(),
                                SUPPORTING_FONT_SIZE,
                            )
                            .with_color(theme.sub_text_color(theme.surface_1()).into())
                            .finish(),
                        )
                        .finish(),
                    )
                    .with_child(ChildView::new(&self.choose_folder_button).finish())
                    .finish(),
            )
            .with_margin_top(12.)
            .finish(),
        );
        body.add_child(
            Container::new(
                Text::new_inline(
                    format!("Final path: {}", destination.to_string_lossy()),
                    appearance.ui_font_family(),
                    SUPPORTING_FONT_SIZE,
                )
                .with_color(theme.sub_text_color(theme.surface_1()).into())
                .finish(),
            )
            .with_margin_top(6.)
            .finish(),
        );
        if let Some(error) = &self.error {
            body.add_child(
                Container::new(
                    Text::new(
                        error.clone(),
                        appearance.ui_font_family(),
                        SUPPORTING_FONT_SIZE,
                    )
                    .with_color(theme.ui_error_color().into())
                    .finish(),
                )
                .with_margin_top(8.)
                .finish(),
            );
        }
        body.finish()
    }
}

impl Entity for CreateWorktreeDialog {
    type Event = CreateWorktreeDialogEvent;
}

impl View for CreateWorktreeDialog {
    fn ui_name() -> &'static str {
        "CreateWorktreeDialog"
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            ctx.focus(&self.name_editor);
        }
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        if !self.visible {
            return Empty::new().finish();
        }
        let appearance = Appearance::as_ref(app);
        Dialog::new(
            "Create worktree".to_string(),
            Some("Create an isolated checkout for another task or agent.".to_string()),
            dialog_styles(appearance),
        )
        .with_child(self.render_body(app, appearance))
        .with_bottom_row_child(ChildView::new(&self.cancel_button).finish())
        .with_bottom_row_child(
            Container::new(ChildView::new(&self.create_button).finish())
                .with_margin_left(8.)
                .finish(),
        )
        .with_width(DIALOG_WIDTH)
        .build()
        .prevent_interaction_with_other_elements()
        .on_dismiss(|ctx, _| ctx.dispatch_typed_action(CreateWorktreeDialogAction::Cancel))
        .finish()
    }
}

impl TypedActionView for CreateWorktreeDialog {
    type Action = CreateWorktreeDialogAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            CreateWorktreeDialogAction::Cancel => ctx.emit(CreateWorktreeDialogEvent::Dismissed),
            CreateWorktreeDialogAction::ChooseFolder => self.choose_folder(ctx),
            CreateWorktreeDialogAction::FolderChosen(path) => {
                self.parent_directory = path.clone();
                self.error = None;
                self.sync_controls(ctx);
                ctx.notify();
            }
            CreateWorktreeDialogAction::Create => self.submit(ctx),
            CreateWorktreeDialogAction::SetMode(mode) => self.set_mode(mode.clone(), ctx),
        }
    }
}
