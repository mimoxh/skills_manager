use crate::{
    error::{AppError, AppResult},
    models::{
        AgentProfile, AgentType, ConflictPolicy, DiscoveryPathEntry, GroupedSkill, InstallResult,
    },
    service::AppService,
};
use eframe::egui::{
    self, Align, Align2, Button, Color32, Context, FontData, FontDefinitions, FontFamily, Frame,
    Layout, Margin, RichText, Rounding, ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2, Visuals,
};
use std::{
    collections::HashSet,
    path::Path,
};

mod theme {
    use super::*;

    pub const APP_BG: Color32 = Color32::from_rgb(247, 247, 245);
    pub const PANEL: Color32 = Color32::from_rgb(255, 255, 255);
    pub const PANEL_SOFT: Color32 = Color32::from_rgb(251, 250, 248);
    pub const BORDER: Color32 = Color32::from_rgb(229, 225, 218);
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(207, 201, 191);
    pub const TEXT: Color32 = Color32::from_rgb(31, 35, 40);
    pub const MUTED: Color32 = Color32::from_rgb(107, 114, 128);
    pub const FAINT: Color32 = Color32::from_rgb(148, 153, 162);
    pub const ACCENT: Color32 = Color32::from_rgb(19, 61, 59);
    pub const ACCENT_SOFT: Color32 = Color32::from_rgb(235, 245, 242);
    pub fn apply(ctx: &Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals = Visuals::light();
        style.visuals.window_fill = APP_BG;
        style.visuals.panel_fill = APP_BG;
        style.visuals.extreme_bg_color = Color32::from_rgb(241, 239, 235);
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = ACCENT_SOFT;
        style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.spacing.window_margin = Margin::same(0.0);
        style.visuals.widgets.inactive.bg_fill = PANEL;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(248, 247, 244);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
        style.visuals.widgets.active.bg_fill = ACCENT_SOFT;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        ctx.set_style(style);
    }

    pub fn panel_frame() -> Frame {
        Frame::none()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::same(16.0))
    }

    pub fn soft_frame() -> Frame {
        Frame::none()
            .fill(PANEL_SOFT)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::same(14.0))
    }

    pub fn list_item_frame(selected: bool, hovered: bool) -> Frame {
        let fill = if selected {
            ACCENT_SOFT
        } else if hovered {
            Color32::from_rgb(249, 248, 245)
        } else {
            PANEL
        };
        let stroke = if selected {
            Stroke::new(1.0, ACCENT)
        } else {
            Stroke::new(1.0, BORDER)
        };
        Frame::none()
            .fill(fill)
            .stroke(stroke)
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::same(12.0))
    }
}

const SIDEBAR_WIDTH: f32 = 220.0;
const TITLEBAR_HEIGHT: f32 = 38.0;
const WORKSPACE_PADDING: f32 = 16.0;
const CONTENT_GAP: f32 = 12.0;
const DETAIL_PANEL_MIN_WIDTH: f32 = 280.0;
const DETAIL_PANEL_MAX_WIDTH: f32 = 360.0;
const WINDOW_BUTTON_WIDTH: f32 = 46.0;
const PANEL_HORIZONTAL_MARGIN: f32 = 32.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Overview,
    Skills,
    Agents,
    Sync,
    Settings,
}

pub struct NativeSkillsApp {
    service: AppService,
    view: View,
    repository: String,
    message: String,
    skills: Vec<GroupedSkill>,
    agents: Vec<AgentProfile>,
    selected_skills: HashSet<String>,
    selected_agents: HashSet<String>,
    skill_search: String,
    global_search: String,
    custom_agent_name: String,
    custom_agent_path: String,
    conflict_policy: ConflictPolicy,
    results: Vec<InstallResult>,
    discovery_paths: Vec<DiscoveryPathEntry>,
    new_discovery_path: String,
    new_discovery_label: String,
    new_discovery_subdir: String,
    sync_popup_open: bool,
    sync_popup_skill: Option<GroupedSkill>,
    sync_popup_agents: HashSet<String>,
    sync_popup_policy: ConflictPolicy,
}

impl NativeSkillsApp {
    fn new(cc: &eframe::CreationContext<'_>) -> AppResult<Self> {
        configure_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        let service = AppService::new()?;
        let mut app = Self {
            service,
            view: View::Overview,
            repository: String::new(),
            message: "正在加载应用状态...".to_string(),
            skills: Vec::new(),
            agents: Vec::new(),
            selected_skills: HashSet::new(),
            selected_agents: HashSet::new(),
            skill_search: String::new(),
            global_search: String::new(),
            custom_agent_name: String::new(),
            custom_agent_path: String::new(),
            conflict_policy: ConflictPolicy::Prompt,
            results: Vec::new(),
            discovery_paths: Vec::new(),
            new_discovery_path: String::new(),
            new_discovery_label: String::new(),
            new_discovery_subdir: String::new(),
            sync_popup_open: false,
            sync_popup_skill: None,
            sync_popup_agents: HashSet::new(),
            sync_popup_policy: ConflictPolicy::BackupOverwrite,
        };
        app.refresh();
        Ok(app)
    }

    fn refresh(&mut self) {
        match self.load_data() {
            Ok(()) => {
                self.message = format!(
                    "已从 Agent 目录识别 {} 个去重 skills，{} 个 agent 配置。",
                    self.skills.len(),
                    self.agents.len()
                );
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn load_data(&mut self) -> AppResult<()> {
        self.repository = self.service.get_repository()?;
        self.agents = self.service.list_agents()?;
        self.skills = self.service.scan_agent_skills().unwrap_or_default();
        self.discovery_paths = self.service.list_discovery_paths()?;
        self.selected_skills
            .retain(|title| self.skills.iter().any(|skill| skill.title == *title));
        self.selected_agents
            .retain(|id| self.agents.iter().any(|agent| agent.id == *id));
        Ok(())
    }

    fn save_repository(&mut self) {
        match self.service.set_repository(&self.repository) {
            Ok(path) => {
                self.repository = path;
                self.refresh();
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn choose_repository(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.repository = path.to_string_lossy().to_string();
            self.save_repository();
        }
    }

    fn import_folder_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.import_path(&path);
        }
    }

    fn import_zip_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Skill 压缩包", &["zip"])
            .pick_file()
        {
            self.import_path(&path);
        }
    }

    fn import_path(&mut self, path: &Path) {
        let result = if path.is_dir() {
            self.service.import_folder(path)
        } else if is_zip(path) {
            self.service.import_zip_file(path)
        } else {
            Err(AppError::Message(
                "只支持导入文件夹或 .zip 压缩包。".to_string(),
            ))
        };

        match result {
            Ok(result) => {
                self.message = result.message;
                let _ = self.load_data();
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn add_custom_agent(&mut self) {
        let name = self.custom_agent_name.trim();
        let path = self.custom_agent_path.trim();
        if name.is_empty() || path.is_empty() {
            self.message = "自定义 Agent 需要名称和 Skills 安装目录。".to_string();
            return;
        }

        let profile = AgentProfile {
            id: format!("custom:{}", path),
            name: name.to_string(),
            agent_type: AgentType::Custom,
            skills_path: path.to_string(),
            adapter_config: None,
        };
        match self.service.add_agent(profile) {
            Ok(agent) => {
                self.custom_agent_name.clear();
                self.custom_agent_path.clear();
                self.selected_agents.insert(agent.id);
                self.refresh();
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn install_selected(&mut self) {
        if self.selected_skills.is_empty() || self.selected_agents.is_empty() {
            self.message = "请至少选择一个 skill 和一个 agent。".to_string();
            return;
        }

        let mut results = Vec::new();
        for title in self.selected_skills.iter().cloned().collect::<Vec<_>>() {
            match self.service.sync_grouped_skill(
                &title,
                None,
                self.selected_agents.iter().cloned().collect(),
                self.conflict_policy.clone(),
            ) {
                Ok(mut next) => results.append(&mut next),
                Err(error) => {
                    self.message = error.to_string();
                    return;
                }
            }
        }
        {
                self.message = format!("完成 {} 个同步任务。", results.len());
                self.results = results;
                let _ = self.load_data();
        }
    }

    fn open_sync_popup(&mut self, skill: GroupedSkill) {
        self.sync_popup_agents = skill.installed_agent_ids.iter().cloned().collect();
        self.sync_popup_policy = ConflictPolicy::BackupOverwrite;
        self.sync_popup_skill = Some(skill);
        self.sync_popup_open = true;
    }

    fn execute_sync_popup(&mut self) {
        let Some(skill) = &self.sync_popup_skill else {
            return;
        };
        let title = skill.title.clone();
        let agent_ids: Vec<String> = self.sync_popup_agents.iter().cloned().collect();
        if agent_ids.is_empty() {
            self.message = "请至少选择一个 Agent。".to_string();
            return;
        }
        match self.service.sync_grouped_skill(
            &title,
            None,
            agent_ids,
            self.sync_popup_policy.clone(),
        ) {
            Ok(results) => {
                self.message = format!("完成 {} 个同步任务。", results.len());
                self.results = results;
                self.sync_popup_open = false;
                self.sync_popup_skill = None;
                let _ = self.load_data();
            }
            Err(error) => {
                self.message = error.to_string();
            }
        }
    }

    fn handle_drops(&mut self, ctx: &Context) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        for file in dropped {
            if let Some(path) = file.path {
                self.import_path(&path);
            }
        }
    }

    fn is_drag_hovering(&self, ctx: &Context) -> bool {
        ctx.input(|input| !input.raw.hovered_files.is_empty())
    }
}

impl eframe::App for NativeSkillsApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_drops(ctx);

        egui::TopBottomPanel::top("titlebar")
            .frame(Frame::none().fill(theme::PANEL).inner_margin(Margin::same(0.0)))
            .show(ctx, |ui| {
                self.titlebar(ui, ctx);
            });

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(SIDEBAR_WIDTH)
            .frame(
                Frame::none()
                    .fill(theme::PANEL)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .rounding(Rounding::same(0.0))
                    .inner_margin(Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                self.sidebar(ui);
            });

        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(theme::APP_BG)
                    .inner_margin(Margin::same(WORKSPACE_PADDING)),
            )
            .show(ctx, |ui| {
                self.command_bar(ui);
                ui.add_space(CONTENT_GAP);
                match self.view {
                    View::Overview => self.overview(ui),
                    View::Skills => self.skills_view(ui),
                    View::Agents => self.agents_view(ui),
                    View::Sync => self.sync_view(ui),
                    View::Settings => self.settings_view(ui),
                }
            });

        if self.sync_popup_open {
            let mut open = self.sync_popup_open;
            let mut should_execute = false;
            let mut should_close = false;

            egui::Window::new("同步 Skill 到 Agents")
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .fixed_size(Vec2::new(480.0, 420.0))
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(skill) = self.sync_popup_skill.clone() {
                        ui.label(
                            RichText::new(&skill.title)
                                .size(18.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                        ui.label(
                            RichText::new(format!(
                                "来源 {} · {} 个副本",
                                skill.best_copy.agent_name,
                                skill.copies.len()
                            ))
                            .color(theme::MUTED),
                        );
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        ui.label(RichText::new("选择目标 Agents").strong());
                        ui.add_space(4.0);

                        ScrollArea::vertical()
                            .id_salt("sync_popup_agents")
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for agent in &self.agents {
                                    let is_installed = skill.installed_agent_ids.contains(&agent.id);
                                    let mut checked =
                                        self.sync_popup_agents.contains(&agent.id);
                                    ui.horizontal(|ui| {
                                        if ui.checkbox(&mut checked, "").changed() {
                                            if checked {
                                                self.sync_popup_agents.insert(agent.id.clone());
                                            } else {
                                                self.sync_popup_agents.remove(&agent.id);
                                            }
                                        }
                                        ui.label(
                                            RichText::new(&agent.name)
                                                .color(theme::TEXT),
                                        );
                                        if is_installed {
                                            pill(
                                                ui,
                                                "已安装",
                                                theme::ACCENT_SOFT,
                                                theme::ACCENT,
                                            );
                                        } else {
                                            pill(
                                                ui,
                                                "未安装",
                                                theme::PANEL_SOFT,
                                                theme::MUTED,
                                            );
                                        }
                                    });
                                }
                            });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        ui.label(RichText::new("冲突策略").strong());
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            policy_button(
                                ui,
                                &mut self.sync_popup_policy,
                                ConflictPolicy::BackupOverwrite,
                                "备份覆盖",
                            );
                            policy_button(
                                ui,
                                &mut self.sync_popup_policy,
                                ConflictPolicy::Skip,
                                "跳过冲突",
                            );
                            policy_button(
                                ui,
                                &mut self.sync_popup_policy,
                                ConflictPolicy::Rename,
                                "另存副本",
                            );
                        });

                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if primary_button(ui, "同步").clicked() {
                                should_execute = true;
                            }
                            if secondary_button(ui, "取消").clicked() {
                                should_close = true;
                            }
                            ui.label(
                                RichText::new(format!(
                                    "{} 个 Agent 已选",
                                    self.sync_popup_agents.len()
                                ))
                                .color(theme::MUTED),
                            );
                        });
                    }
                });

            if !open {
                should_close = true;
            }
            if should_execute {
                self.execute_sync_popup();
            }
            if should_close {
                self.sync_popup_open = false;
                self.sync_popup_skill = None;
            }
        }

        if self.is_drag_hovering(ctx) {
            egui::Area::new(egui::Id::new("drop_overlay"))
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 245))
                        .stroke(Stroke::new(1.0, theme::ACCENT))
                        .rounding(Rounding::same(16.0))
                        .inner_margin(Margin::same(24.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("松开即可导入")
                                    .size(20.0)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            ui.label(RichText::new("支持文件夹和 .zip 压缩包").color(theme::MUTED));
                        });
                });
        }

    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::APP_BG.to_normalized_gamma_f32()
    }
}

impl NativeSkillsApp {
    fn titlebar(&mut self, ui: &mut Ui, ctx: &Context) {
        let available = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available, TITLEBAR_HEIGHT),
            Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, Rounding::same(0.0), theme::PANEL);
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, theme::BORDER),
        );

        if response.double_clicked() {
            let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        } else if response.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        ui.allocate_new_ui(
            egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(10.0, 0.0))),
            |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(TITLEBAR_HEIGHT);
                    ui.add_space(2.0);
                    title_mark(ui);
                    ui.label(RichText::new("Skills Manager").strong().color(theme::TEXT));
                    let spacer = (ui.available_width() - WINDOW_BUTTON_WIDTH * 3.0).max(0.0);
                    ui.add_space(spacer);
                    if window_button(ui, "−").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                    if window_button(ui, if maximized { "▢" } else { "□" }).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    if close_button(ui).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            },
        );
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Skills Manager").size(20.0).strong());
        ui.label(RichText::new("原生工作台").color(theme::MUTED));
        ui.add_space(18.0);
        nav_button(ui, &mut self.view, View::Overview, "概览");
        nav_button(ui, &mut self.view, View::Skills, "Skills");
        nav_button(ui, &mut self.view, View::Agents, "Agents");
        nav_button(ui, &mut self.view, View::Sync, "同步");
        nav_button(ui, &mut self.view, View::Settings, "设置");
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.label(RichText::new("v0.1.0").color(theme::FAINT));
            ui.label(RichText::new("Native egui").color(theme::FAINT));
            pill(ui, "无需浏览器", theme::ACCENT_SOFT, theme::ACCENT);
            ui.add_space(8.0);
            ui.label(RichText::new(short_path(&self.repository)).color(theme::MUTED));
            ui.label(RichText::new("主仓库").color(theme::FAINT));
        });
    }

    fn command_bar(&mut self, ui: &mut Ui) {
        theme::panel_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("工作台").size(18.0).strong());
                ui.add_space(8.0);
                let search_width = (ui.available_width() - 360.0).clamp(220.0, 360.0);
                ui.add_sized(
                    [search_width, 34.0],
                    TextEdit::singleline(&mut self.global_search).hint_text("搜索或过滤当前内容"),
                );
                if secondary_button(ui, "导入文件夹").clicked() {
                    self.import_folder_dialog();
                }
                if secondary_button(ui, "导入 zip").clicked() {
                    self.import_zip_dialog();
                }
                if secondary_button(ui, "刷新").clicked() {
                    self.refresh();
                }
                if primary_button(ui, "同步").clicked() {
                    self.view = View::Sync;
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                clipped_pill(ui, &self.message, theme::PANEL_SOFT, theme::MUTED);
                ui.label(RichText::new("主仓库").color(theme::FAINT));
                ui.label(RichText::new(short_path(&self.repository)).color(theme::MUTED));
            });
        });
    }

    fn overview(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            metric_card(
                ui,
                "Skills",
                self.skills.len().to_string(),
                "按标题去重后的技能",
            );
            metric_card(
                ui,
                "Agents",
                self.agents.len().to_string(),
                "可同步的目标配置",
            );
            metric_card(
                ui,
                "待同步",
                self.skills
                    .iter()
                    .map(|skill| skill.missing_agent_ids.len())
                    .sum::<usize>()
                    .to_string(),
                "Agent 缺失的副本",
            );
        });
        ui.add_space(12.0);
        let total_width = ui.available_width();
        let stacked = is_stacked(total_width);
        let (left_width, right_width) = content_widths(total_width);
        if stacked {
            ScrollArea::vertical()
                .id_salt("overview_stacked")
                .show(ui, |ui| {
                theme::panel_frame().show(ui, |left| {
                    left.set_width(left_width);
                    section_header(left, "最近状态", "导入、扫描和同步结果会显示在这里");
                    empty_or_results(left, &self.results, &self.message);
                });
                ui.add_space(CONTENT_GAP);
                theme::panel_frame().show(ui, |right| {
                    right.set_width(right_width);
                    section_header(right, "快速操作", "从这里开始导入和同步");
                    action_panel(
                        right,
                        "导入 Skills",
                        "选择文件夹或 zip 压缩包，也可以直接拖入窗口。",
                    );
                    right.horizontal(|ui| {
                        if primary_button(ui, "导入文件夹").clicked() {
                            self.import_folder_dialog();
                        }
                        if secondary_button(ui, "导入 zip").clicked() {
                            self.import_zip_dialog();
                        }
                    });
                    right.add_space(10.0);
                    action_panel(right, "同步到 Agent", "选择 skills 和 agents 后执行同步。");
                    if primary_button(right, "打开同步页").clicked() {
                        self.view = View::Sync;
                    }
                });
            });
        } else {
            ui.horizontal(|ui| {
                theme::panel_frame().show(ui, |left| {
                    left.set_width(left_width);
                    section_header(left, "最近状态", "导入、扫描和同步结果会显示在这里");
                    empty_or_results(left, &self.results, &self.message);
                });
                ui.add_space(CONTENT_GAP);
                theme::panel_frame().show(ui, |right| {
                    right.set_width(right_width);
                    section_header(right, "快速操作", "从这里开始导入和同步");
                    action_panel(
                        right,
                        "导入 Skills",
                        "选择文件夹或 zip 压缩包，也可以直接拖入窗口。",
                    );
                    right.horizontal(|ui| {
                        if primary_button(ui, "导入文件夹").clicked() {
                            self.import_folder_dialog();
                        }
                        if secondary_button(ui, "导入 zip").clicked() {
                            self.import_zip_dialog();
                        }
                    });
                    right.add_space(10.0);
                    action_panel(right, "同步到 Agent", "选择 skills 和 agents 后执行同步。");
                    if primary_button(right, "打开同步页").clicked() {
                        self.view = View::Sync;
                    }
                });
            });
        }
    }

    fn skills_view(&mut self, ui: &mut Ui) {
        section_header(ui, "Skills", "点击 skill 即可选择 Agents 进行同步");
        ui.add_sized(
            [ui.available_width(), 34.0],
            TextEdit::singleline(&mut self.skill_search)
                .hint_text("搜索 skill 标题、来源 Agent 或路径"),
        );
        ui.add_space(8.0);
        if self.skills.is_empty() {
            empty_state(
                ui,
                "还没有从 Agent 目录识别到 skills。",
                "添加自定义 Agent，或确认 Codex/Claude skills 目录已存在后点击刷新。",
            );
            ui.horizontal(|ui| {
                if primary_button(ui, "导入文件夹").clicked() {
                    self.import_folder_dialog();
                }
                if secondary_button(ui, "导入 zip").clicked() {
                    self.import_zip_dialog();
                }
            });
        } else {
            let query = active_query(&self.skill_search, &self.global_search);
            let skills = self.skills.clone();
            ScrollArea::vertical()
                .id_salt("skills_list")
                .show(ui, |ui| {
                for skill in skills {
                    if !skill_matches(&skill, &query) {
                        continue;
                    }
                    if skill_card(ui, &skill, false).clicked() {
                        self.open_sync_popup(skill.clone());
                    }
                }
            });
        }
    }

    fn agents_view(&mut self, ui: &mut Ui) {
        let total_width = ui.available_width();
        let stacked = is_stacked(total_width);
        let (left_width, right_width) = content_widths(total_width);
        let left_content = |left: &mut Ui, app: &mut Self| {
            left.set_width(left_width);
            section_header(left, "Agents", "选择要安装 skills 的目标");
            let agents = app.agents.clone();
            ScrollArea::vertical()
                .id_salt("agents_list")
                .show(left, |ui| {
                if agents.is_empty() {
                    empty_state(ui, "没有发现 agent。", "可以在右侧添加自定义 agent。");
                }
                for agent in agents {
                    let selected = app.selected_agents.contains(&agent.id);
                    let installed = app
                        .skills
                        .iter()
                        .filter(|skill| skill.installed_agent_ids.contains(&agent.id))
                        .count();
                    let missing = app
                        .skills
                        .iter()
                        .filter(|skill| skill.missing_agent_ids.contains(&agent.id))
                        .count();
                    if agent_card(ui, &agent, selected, installed, missing).clicked() {
                        toggle(&mut app.selected_agents, &agent.id);
                    }
                }
            });
        };
        let right_content = |right: &mut Ui, app: &mut Self| {
            right.set_width(right_width);
            section_header(right, "自定义 Agent", "添加一个普通目录作为同步目标");
            label_input(right, "名称", &mut app.custom_agent_name, "例如 My Agent");
            right.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Skills 安装目录").color(theme::MUTED));
                    ui.add_sized(
                        [ui.available_width().clamp(220.0, 520.0), 34.0],
                        TextEdit::singleline(&mut app.custom_agent_path)
                            .hint_text("选择或输入目录"),
                    );
                });
                if secondary_button(ui, "选择").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        app.custom_agent_path = path.to_string_lossy().to_string();
                    }
                }
            });
            if primary_button(right, "添加 Agent").clicked() {
                app.add_custom_agent();
            }
            right.add_space(16.0);
            pill(
                right,
                &format!("已选择 {} 个 agents", app.selected_agents.len()),
                theme::ACCENT_SOFT,
                theme::ACCENT,
            );
        };
        if stacked {
            ScrollArea::vertical()
                .id_salt("agents_stacked")
                .show(ui, |ui| {
                theme::panel_frame().show(ui, |left| left_content(left, self));
                ui.add_space(CONTENT_GAP);
                theme::panel_frame().show(ui, |right| right_content(right, self));
            });
        } else {
            ui.horizontal(|ui| {
                theme::panel_frame().show(ui, |left| left_content(left, self));
                ui.add_space(CONTENT_GAP);
                theme::panel_frame().show(ui, |right| right_content(right, self));
            });
        }
    }

    fn sync_view(&mut self, ui: &mut Ui) {
        let total_width = ui.available_width();
        let stacked = is_stacked(total_width);
        let (left_width, right_width) = content_widths(total_width);
        let left_content = |left: &mut Ui, app: &mut Self| {
            left.set_width(left_width);
            section_header(left, "同步矩阵", "检查每个 skill 到每个 agent 的状态");
            left.horizontal(|ui| {
                policy_button(
                    ui,
                    &mut app.conflict_policy,
                    ConflictPolicy::Prompt,
                    "提示冲突",
                );
                policy_button(
                    ui,
                    &mut app.conflict_policy,
                    ConflictPolicy::BackupOverwrite,
                    "备份覆盖",
                );
                policy_button(
                    ui,
                    &mut app.conflict_policy,
                    ConflictPolicy::Skip,
                    "跳过冲突",
                );
                policy_button(
                    ui,
                    &mut app.conflict_policy,
                    ConflictPolicy::Rename,
                    "另存副本",
                );
            });
            left.add_space(8.0);
            ScrollArea::vertical()
                .id_salt("sync_matrix")
                .show(left, |ui| {
                if app.selected_skills.is_empty() || app.selected_agents.is_empty() {
                    empty_state(
                        ui,
                        "还没有同步矩阵",
                        "请先在 Skills 和 Agents 页面选择项目。",
                    );
                }
                for title in &app.selected_skills {
                    let skill = app.skills.iter().find(|skill| skill.title == *title);
                    for agent_id in &app.selected_agents {
                        let installed = skill
                            .map(|skill| skill.installed_agent_ids.contains(agent_id))
                            .unwrap_or(false);
                        sync_row(ui, title, &agent_name(&app.agents, agent_id), installed);
                    }
                }
            });
        };
        let right_content = |right: &mut Ui, app: &mut Self| {
            right.set_width(right_width);
            section_header(right, "执行", "同步选中的组合");
            detail_row(right, "Skills", &app.selected_skills.len().to_string());
            detail_row(right, "Agents", &app.selected_agents.len().to_string());
            detail_row(right, "策略", conflict_policy_label(&app.conflict_policy));
            if primary_button(right, "执行同步").clicked() {
                app.install_selected();
            }
            right.add_space(16.0);
            section_header(right, "最近结果", "");
            ScrollArea::vertical()
                .id_salt("sync_results")
                .max_height(280.0)
                .show(right, |ui| {
                if app.results.is_empty() {
                    empty_state(ui, "暂无同步结果", "执行同步后会显示安装、更新或跳过记录。");
                }
                for result in &app.results {
                    result_card(ui, result);
                }
            });
        };
        if stacked {
            ScrollArea::vertical()
                .id_salt("sync_stacked")
                .show(ui, |ui| {
                theme::panel_frame().show(ui, |left| left_content(left, self));
                ui.add_space(CONTENT_GAP);
                theme::panel_frame().show(ui, |right| right_content(right, self));
            });
        } else {
            ui.horizontal(|ui| {
                theme::panel_frame().show(ui, |left| left_content(left, self));
                ui.add_space(CONTENT_GAP);
                theme::panel_frame().show(ui, |right| right_content(right, self));
            });
        }
    }

    fn settings_view(&mut self, ui: &mut Ui) {
        let total_width = ui.available_width();
        let stacked = is_stacked(total_width);
        let (left_width, right_width) = content_widths(total_width);
        let data_dir = self.service.data_dir().to_string_lossy().to_string();
        let backup_root = self.service.backup_root().to_string_lossy().to_string();
        let import_root = self.service.import_root().to_string_lossy().to_string();
        let left_content = |left: &mut Ui, app: &mut Self| {
            left.set_width(left_width);
            section_header(left, "路径", "检查应用使用的本地目录");
            label_input(left, "主仓库", &mut app.repository, "Skills 主仓库路径");
            left.horizontal(|ui| {
                if secondary_button(ui, "选择目录").clicked() {
                    app.choose_repository();
                }
                if primary_button(ui, "保存主仓库").clicked() {
                    app.save_repository();
                }
            });
            left.add_space(12.0);
            path_row(left, "应用数据", &data_dir);
            path_row(left, "备份目录", &backup_root);
            path_row(left, "导入缓存", &import_root);
            left.add_space(16.0);
            section_header(left, "关于", "当前运行的是原生桌面版");
            detail_row(left, "UI", "egui / eframe");
            detail_row(left, "运行方式", "不依赖浏览器或 localhost");
            detail_row(left, "版本", "0.1.0");
            action_panel(
                left,
                "便携版",
                "使用 scripts/build-portable.ps1 生成 Windows 便携版目录。",
            );
        };
        let right_content = |right: &mut Ui, app: &mut Self| {
            right.set_width(right_width);
            section_header(
                right,
                "Agent 发现路径",
                "添加额外的 agent skills 目录用于自动识别",
            );
            label_input(right, "路径", &mut app.new_discovery_path, "例如 C:\\Users\\me\\.cursor");
            label_input(right, "标签", &mut app.new_discovery_label, "例如 Cursor");
            label_input(right, "Skills 子目录", &mut app.new_discovery_subdir, "默认 skills");
            right.add_space(6.0);
            if primary_button(right, "添加发现路径").clicked() {
                let path = app.new_discovery_path.trim().to_string();
                let label = app.new_discovery_label.trim().to_string();
                let subdir = if app.new_discovery_subdir.trim().is_empty() {
                    "skills".to_string()
                } else {
                    app.new_discovery_subdir.trim().to_string()
                };
                match app.service.add_discovery_path(&path, &label, &subdir) {
                    Ok(()) => {
                        app.new_discovery_path.clear();
                        app.new_discovery_label.clear();
                        app.new_discovery_subdir.clear();
                        app.message = "已添加发现路径".to_string();
                        let _ = app.load_data();
                    }
                    Err(error) => app.message = error.to_string(),
                }
            }
            right.add_space(12.0);
            section_header(right, "已配置的发现路径", "");
            if app.discovery_paths.is_empty() {
                empty_state(
                    right,
                    "还没有配置发现路径",
                    "添加包含 skills 的 agent 目录路径。",
                );
            } else {
                ScrollArea::vertical()
                    .id_salt("discovery_paths")
                    .max_height(320.0)
                    .show(right, |ui| {
                    let paths = app.discovery_paths.clone();
                    for entry in &paths {
                        theme::soft_frame().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&entry.label).strong());
                                    ui.label(
                                        RichText::new(short_path(&entry.path)).color(theme::MUTED),
                                    );
                                    ui.label(
                                        RichText::new(format!("子目录: {}", entry.skills_subdir))
                                            .color(theme::FAINT)
                                            .size(11.0),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if secondary_button(ui, "删除").clicked() {
                                        let _ = app.service.remove_discovery_path(&entry.path);
                                        app.message = format!("已删除发现路径: {}", entry.label);
                                        let _ = app.load_data();
                                    }
                                });
                            });
                        });
                    }
                });
            }
        };
        if stacked {
            ScrollArea::vertical()
                .id_salt("settings_stacked")
                .show(ui, |ui| {
                theme::panel_frame().show(ui, |left| left_content(left, self));
                ui.add_space(CONTENT_GAP);
                theme::panel_frame().show(ui, |right| right_content(right, self));
            });
        } else {
            ui.horizontal(|ui| {
                theme::panel_frame().show(ui, |left| left_content(left, self));
                ui.add_space(CONTENT_GAP);
                theme::panel_frame().show(ui, |right| right_content(right, self));
            });
        }
    }
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Skills Manager")
            .with_inner_size(Vec2::new(1280.0, 820.0))
            .with_min_inner_size(Vec2::new(960.0, 680.0))
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "Skills Manager",
        options,
        Box::new(|cc| {
            NativeSkillsApp::new(cc)
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)
        }),
    )
}

fn configure_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    for path in [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("system_cjk".to_string(), FontData::from_owned(bytes));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "system_cjk".to_string());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, "system_cjk".to_string());
            ctx.set_fonts(fonts);
            return;
        }
    }
    ctx.set_fonts(fonts);
}

fn title_mark(ui: &mut Ui) {
    Frame::none()
        .fill(theme::ACCENT)
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::same(0.0))
        .show(ui, |ui| {
            ui.add_sized(
                [22.0, 22.0],
                egui::Label::new(RichText::new("S").strong().color(Color32::WHITE).size(13.0)),
            );
        });
}

fn window_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [WINDOW_BUTTON_WIDTH, TITLEBAR_HEIGHT - 1.0],
        Button::new(RichText::new(label).size(15.0).color(theme::TEXT))
            .fill(theme::PANEL)
            .stroke(Stroke::NONE)
            .rounding(Rounding::same(0.0)),
    )
}

fn close_button(ui: &mut Ui) -> egui::Response {
    ui.add_sized(
        [WINDOW_BUTTON_WIDTH, TITLEBAR_HEIGHT - 1.0],
        Button::new(RichText::new("×").size(16.0).color(theme::TEXT))
            .fill(theme::PANEL)
            .stroke(Stroke::NONE)
            .rounding(Rounding::same(0.0)),
    )
}

fn nav_button(ui: &mut Ui, view: &mut View, target: View, label: &str) {
    let selected = *view == target;
    let response = ui.add_sized(
        [ui.available_width(), 36.0],
        Button::new(RichText::new(label).color(if selected { theme::ACCENT } else { theme::TEXT }))
            .fill(if selected {
                theme::ACCENT_SOFT
            } else {
                theme::PANEL
            })
            .stroke(Stroke::new(
                1.0,
                if selected {
                    theme::ACCENT
                } else {
                    theme::BORDER
                },
            ))
            .rounding(Rounding::same(9.0)),
    );
    if response.clicked() {
        *view = target;
    }
}

fn content_widths(total: f32) -> (f32, f32) {
    if is_stacked(total) {
        let width = (total - PANEL_HORIZONTAL_MARGIN).max(0.0);
        return (width, width);
    }
    let right_outer = (total * 0.32).clamp(DETAIL_PANEL_MIN_WIDTH, DETAIL_PANEL_MAX_WIDTH);
    let left_outer = (total - CONTENT_GAP - right_outer).max(0.0);
    let left = (left_outer - PANEL_HORIZONTAL_MARGIN).max(0.0);
    let right = (right_outer - PANEL_HORIZONTAL_MARGIN).max(0.0);
    (left, right)
}

fn is_stacked(_total: f32) -> bool {
    true
}

fn primary_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [button_width(label), 36.0],
        Button::new(RichText::new(label).color(Color32::WHITE))
            .fill(theme::ACCENT)
            .stroke(Stroke::new(1.0, theme::ACCENT))
            .rounding(Rounding::same(8.0)),
    )
}

fn secondary_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add_sized(
        [button_width(label), 36.0],
        Button::new(RichText::new(label).color(theme::TEXT))
            .fill(theme::PANEL)
            .stroke(Stroke::new(1.0, theme::BORDER_STRONG))
            .rounding(Rounding::same(8.0)),
    )
}

fn policy_button(ui: &mut Ui, current: &mut ConflictPolicy, value: ConflictPolicy, label: &str) {
    let selected = *current == value;
    let response = ui.add_sized(
        [button_width(label), 34.0],
        Button::new(RichText::new(label).color(if selected {
            theme::ACCENT
        } else {
            theme::MUTED
        }))
        .fill(if selected {
            theme::ACCENT_SOFT
        } else {
            theme::PANEL
        })
        .stroke(Stroke::new(
            1.0,
            if selected {
                theme::ACCENT
            } else {
                theme::BORDER
            },
        ))
        .rounding(Rounding::same(8.0)),
    );
    if response.clicked() {
        *current = value;
    }
}

fn section_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(18.0).strong().color(theme::TEXT));
            if !subtitle.is_empty() {
                ui.label(RichText::new(subtitle).color(theme::MUTED));
            }
        });
    });
    ui.add_space(10.0);
}

fn metric_card(ui: &mut Ui, label: &str, value: String, helper: &str) {
    theme::soft_frame().show(ui, |ui| {
        ui.set_width(220.0);
        ui.label(RichText::new(value).size(28.0).strong().color(theme::TEXT));
        ui.label(RichText::new(label).strong());
        ui.label(RichText::new(helper).color(theme::MUTED));
    });
}

fn skill_card(ui: &mut Ui, skill: &GroupedSkill, selected: bool) -> egui::Response {
    let id = ui.id().with(("skill-card", &skill.title));
    let inner = theme::list_item_frame(selected, false).show(ui, |ui| {
        let width = ui.available_width();
        let action_width = 78.0;
        let text_width = (width - action_width - 18.0).max(160.0);
        ui.set_min_height(62.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(text_width);
                ui.label(RichText::new(short_path(&skill.title)).strong());
                ui.label(
                    RichText::new(format!(
                        "来源 {} · {} 个副本",
                        short_path(&skill.best_copy.agent_name),
                        skill.copies.len()
                    ))
                    .color(theme::MUTED),
                );
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        &skill
                            .best_copy
                            .version
                            .as_deref()
                            .map(|version| format!("v{}", version))
                            .unwrap_or_else(|| "未声明版本".to_string()),
                        theme::PANEL_SOFT,
                        theme::MUTED,
                    );
                    clipped_pill(
                        ui,
                        &format!(
                            "{} 已有 / {} 缺失",
                            skill.installed_agent_ids.len(),
                            skill.missing_agent_ids.len()
                        ),
                        theme::PANEL_SOFT,
                        theme::MUTED,
                    );
                });
            });
            ui.add_space((ui.available_width() - action_width).max(0.0));
            pill(
                ui,
                if selected { "已选择" } else { "选择" },
                if selected {
                    theme::ACCENT_SOFT
                } else {
                    theme::PANEL_SOFT
                },
                if selected {
                    theme::ACCENT
                } else {
                    theme::MUTED
                },
            );
        });
    });
    let response = ui.interact(inner.response.rect, id, Sense::click());
    if response.hovered() && !selected {
        ui.painter().rect_stroke(
            inner.response.rect,
            Rounding::same(10.0),
            Stroke::new(1.0, theme::BORDER_STRONG),
        );
    }
    response
}

fn agent_card(
    ui: &mut Ui,
    agent: &AgentProfile,
    selected: bool,
    installed: usize,
    missing: usize,
) -> egui::Response {
    let id = ui.id().with(("agent-card", &agent.id));
    let inner = theme::list_item_frame(selected, false).show(ui, |ui| {
        let width = ui.available_width();
        let action_width = 86.0;
        let text_width = (width - action_width - 18.0).max(160.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(text_width);
                ui.label(RichText::new(short_path(&agent_label(agent))).strong());
                ui.label(RichText::new(short_path(&agent.skills_path)).color(theme::MUTED));
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        &format!("{} 已有", installed),
                        theme::PANEL_SOFT,
                        theme::MUTED,
                    );
                    pill(
                        ui,
                        &format!("{} 缺失", missing),
                        theme::PANEL_SOFT,
                        theme::MUTED,
                    );
                });
            });
            ui.add_space((ui.available_width() - action_width).max(0.0));
            pill(
                ui,
                if selected {
                    "已选择"
                } else {
                    agent.agent_type.as_str()
                },
                if selected {
                    theme::ACCENT_SOFT
                } else {
                    theme::PANEL_SOFT
                },
                if selected {
                    theme::ACCENT
                } else {
                    theme::MUTED
                },
            );
        });
    });
    let response = ui.interact(inner.response.rect, id, Sense::click());
    if response.hovered() && !selected {
        ui.painter().rect_stroke(
            inner.response.rect,
            Rounding::same(10.0),
            Stroke::new(1.0, theme::BORDER_STRONG),
        );
    }
    response
}

fn sync_row(ui: &mut Ui, skill_title: &str, agent_name: &str, installed: bool) {
    theme::soft_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(skill_title).strong());
            ui.label(RichText::new(agent_name).color(theme::MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (label, color) = if installed {
                    ("已有", theme::ACCENT)
                } else {
                    ("缺失", theme::MUTED)
                };
                pill(ui, label, theme::PANEL, color);
            });
        });
    });
}

fn result_card(ui: &mut Ui, result: &InstallResult) {
    theme::soft_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            pill(ui, &result.action, theme::PANEL, theme::ACCENT);
            ui.label(RichText::new(&result.message).color(theme::TEXT));
        });
        ui.label(RichText::new(&result.target_path).color(theme::MUTED));
    });
}

fn empty_or_results(ui: &mut Ui, results: &[InstallResult], message: &str) {
    if results.is_empty() {
        empty_state(ui, message, "完成导入或同步后会显示更详细的记录。");
    } else {
        ScrollArea::vertical().show(ui, |ui| {
            for result in results {
                result_card(ui, result);
            }
        });
    }
}

fn action_panel(ui: &mut Ui, title: &str, text: &str) {
    theme::soft_frame().show(ui, |ui| {
        ui.label(RichText::new(title).strong());
        ui.label(RichText::new(text).color(theme::MUTED));
    });
}

fn empty_state(ui: &mut Ui, title: &str, body: &str) {
    theme::soft_frame().show(ui, |ui| {
        ui.add_space(6.0);
        ui.label(RichText::new(title).strong().color(theme::TEXT));
        ui.label(RichText::new(body).color(theme::MUTED));
        ui.add_space(6.0);
    });
}


fn detail_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        ui.add_sized(
            [76.0, 24.0],
            egui::Label::new(RichText::new(label).color(theme::MUTED)),
        );
        ui.label(RichText::new(short_path(value)).color(theme::TEXT));
    });
}

fn path_row(ui: &mut Ui, label: &str, path: &str) {
    theme::soft_frame().show(ui, |ui| {
        ui.label(RichText::new(label).color(theme::MUTED));
        ui.label(
            RichText::new(short_path(path))
                .monospace()
                .color(theme::TEXT),
        );
    });
}

fn label_input(ui: &mut Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(RichText::new(label).color(theme::MUTED));
    ui.add_sized(
        [ui.available_width(), 34.0],
        TextEdit::singleline(value).hint_text(hint),
    );
}

fn pill(ui: &mut Ui, text: &str, fill: Color32, color: Color32) {
    pill_text(ui, text, fill, color, None);
}

fn clipped_pill(ui: &mut Ui, text: &str, fill: Color32, color: Color32) {
    let max_width = ui.available_width().clamp(160.0, 560.0);
    pill_text(ui, text, fill, color, Some(max_width));
}

fn pill_text(ui: &mut Ui, text: &str, fill: Color32, color: Color32, max_width: Option<f32>) {
    Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .rounding(Rounding::same(999.0))
        .inner_margin(Margin::symmetric(9.0, 3.0))
        .show(ui, |ui| {
            if let Some(width) = max_width {
                ui.add_sized(
                    [width, 18.0],
                    egui::Label::new(RichText::new(short_path(text)).color(color).size(12.0)),
                );
            } else {
                ui.label(RichText::new(text).color(color).size(12.0));
            }
        });
}

fn button_width(label: &str) -> f32 {
    (label.chars().count() as f32 * 14.0 + 28.0).clamp(62.0, 128.0)
}

fn toggle(set: &mut HashSet<String>, value: &str) {
    if !set.insert(value.to_string()) {
        set.remove(value);
    }
}

fn skill_matches(skill: &GroupedSkill, query: &str) -> bool {
    query.is_empty()
        || skill.title.to_lowercase().contains(query)
        || skill.best_copy.agent_name.to_lowercase().contains(query)
        || skill.best_copy.skill_path.to_lowercase().contains(query)
        || skill
            .copies
            .iter()
            .any(|copy| copy.agent_name.to_lowercase().contains(query))
}

fn active_query(local: &str, global: &str) -> String {
    if local.trim().is_empty() {
        global.trim().to_lowercase()
    } else {
        local.trim().to_lowercase()
    }
}

fn conflict_policy_label(policy: &ConflictPolicy) -> &'static str {
    match policy {
        ConflictPolicy::Prompt => "提示冲突",
        ConflictPolicy::BackupOverwrite => "备份覆盖",
        ConflictPolicy::Skip => "跳过冲突",
        ConflictPolicy::Rename => "另存副本",
    }
}

fn agent_label(agent: &AgentProfile) -> String {
    format!("{} ({})", agent.name, agent.agent_type.as_str())
}

fn agent_name(agents: &[AgentProfile], agent_id: &str) -> String {
    agents
        .iter()
        .find(|agent| agent.id == agent_id)
        .map(|agent| agent.name.clone())
        .unwrap_or_else(|| agent_id.to_string())
}

fn short_path(path: &str) -> String {
    const MAX: usize = 32;
    if path.chars().count() <= MAX {
        return path.to_string();
    }
    let suffix = path
        .chars()
        .rev()
        .take(MAX - 3)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("...{}", suffix)
}

fn is_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}
