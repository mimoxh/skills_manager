use crate::{
    error::{AppError, AppResult},
    models::{
        AgentProfile, AgentType, ConflictPolicy, InstallResult, InstallState, InstallStatus,
        SkillSummary,
    },
    service::AppService,
};
use eframe::egui::{
    self, Align, Align2, Button, Color32, Context, FontData, FontDefinitions, FontFamily, Frame,
    Layout, Margin, RichText, Rounding, ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2, Visuals,
};
use std::{
    collections::{HashMap, HashSet},
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
    pub const WARNING: Color32 = Color32::from_rgb(143, 91, 36);
    pub const DANGER: Color32 = Color32::from_rgb(155, 55, 55);

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
    skills: Vec<SkillSummary>,
    agents: Vec<AgentProfile>,
    states: Vec<InstallState>,
    selected_skills: HashSet<String>,
    selected_agents: HashSet<String>,
    skill_search: String,
    global_search: String,
    custom_agent_name: String,
    custom_agent_path: String,
    conflict_policy: ConflictPolicy,
    results: Vec<InstallResult>,
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
            states: Vec::new(),
            selected_skills: HashSet::new(),
            selected_agents: HashSet::new(),
            skill_search: String::new(),
            global_search: String::new(),
            custom_agent_name: String::new(),
            custom_agent_path: String::new(),
            conflict_policy: ConflictPolicy::Prompt,
            results: Vec::new(),
        };
        app.refresh();
        Ok(app)
    }

    fn refresh(&mut self) {
        match self.load_data() {
            Ok(()) => {
                self.message = format!(
                    "已加载 {} 个 skills，{} 个 agent 配置。",
                    self.skills.len(),
                    self.agents.len()
                );
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn load_data(&mut self) -> AppResult<()> {
        self.repository = self.service.get_repository()?;
        self.skills = self.service.scan_skills().unwrap_or_default();
        self.agents = self.service.list_agents()?;
        self.states = self.service.list_install_state().unwrap_or_default();
        self.selected_skills
            .retain(|id| self.skills.iter().any(|skill| skill.manifest.id == *id));
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

        match self.service.install_skills(
            self.selected_skills.iter().cloned().collect(),
            self.selected_agents.iter().cloned().collect(),
            self.conflict_policy.clone(),
        ) {
            Ok(results) => {
                self.message = format!("完成 {} 个同步任务。", results.len());
                self.results = results;
                let _ = self.load_data();
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn state_by_pair(&self) -> HashMap<String, &InstallState> {
        self.states
            .iter()
            .map(|state| (format!("{}:{}", state.agent_id, state.skill_id), state))
            .collect()
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
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(220.0)
            .frame(
                Frame::none()
                    .fill(theme::PANEL)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(Margin::same(16.0)),
            )
            .show(ctx, |ui| self.sidebar(ui));

        egui::TopBottomPanel::top("command_bar")
            .resizable(false)
            .frame(
                Frame::none()
                    .fill(theme::APP_BG)
                    .inner_margin(Margin::symmetric(14.0, 12.0)),
            )
            .show(ctx, |ui| self.command_bar(ui));

        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(theme::APP_BG)
                    .inner_margin(Margin::symmetric(14.0, 0.0)),
            )
            .show(ctx, |ui| match self.view {
                View::Overview => self.overview(ui),
                View::Skills => self.skills_view(ui),
                View::Agents => self.agents_view(ui),
                View::Sync => self.sync_view(ui),
                View::Settings => self.settings_view(ui),
            });

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
}

impl NativeSkillsApp {
    fn sidebar(&mut self, ui: &mut Ui) {
        ui.set_height(ui.available_height());
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
                ui.add_sized(
                    [260.0, 34.0],
                    TextEdit::singleline(&mut self.global_search).hint_text("搜索或过滤当前内容"),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, "同步").clicked() {
                        self.view = View::Sync;
                    }
                    if secondary_button(ui, "刷新").clicked() {
                        self.refresh();
                    }
                    if secondary_button(ui, "导入 zip").clicked() {
                        self.import_zip_dialog();
                    }
                    if secondary_button(ui, "导入文件夹").clicked() {
                        self.import_folder_dialog();
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                pill(ui, &self.message, theme::PANEL_SOFT, theme::MUTED);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(&self.repository).color(theme::MUTED));
                    ui.label(RichText::new("主仓库").color(theme::FAINT));
                });
            });
        });
    }

    fn overview(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            metric_card(ui, "Skills", self.skills.len().to_string(), "主仓库中可识别的技能");
            metric_card(ui, "Agents", self.agents.len().to_string(), "可同步的目标配置");
            metric_card(
                ui,
                "待同步",
                format!("{} x {}", self.selected_skills.len(), self.selected_agents.len()),
                "当前选择矩阵",
            );
        });
        ui.add_space(12.0);
        let (left_width, right_width) = content_widths(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_height(ui.available_height());
            theme::panel_frame().show(ui, |left| {
                left.set_width(left_width);
                section_header(left, "最近状态", "导入、扫描和同步结果会显示在这里");
                empty_or_results(left, &self.results, &self.message);
            });
            ui.add_space(4.0);
            theme::panel_frame().show(ui, |right| {
                right.set_width(right_width);
                section_header(right, "快速操作", "从这里开始导入和同步");
                action_panel(right, "导入 Skills", "选择文件夹或 zip 压缩包，也可以直接拖入窗口。");
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

    fn skills_view(&mut self, ui: &mut Ui) {
        let (left_width, right_width) = content_widths(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_height(ui.available_height());
            theme::panel_frame().show(ui, |left| {
                left.set_width(left_width);
                section_header(left, "Skills", "管理主仓库中的 skills");
                left.add_sized(
                    [left.available_width(), 34.0],
                    TextEdit::singleline(&mut self.skill_search).hint_text("搜索 skill 名称或 id"),
                );
                left.add_space(8.0);
                if self.skills.is_empty() {
                    empty_state(left, "主仓库里还没有可识别的 skill manifest。", "导入文件夹或 zip 后会出现在这里。");
                    left.horizontal(|ui| {
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
                    ScrollArea::vertical().show(left, |ui| {
                        for skill in skills {
                            if !skill_matches(&skill, &query) {
                                continue;
                            }
                            let selected = self.selected_skills.contains(&skill.manifest.id);
                            if skill_card(ui, &skill, selected).clicked() {
                                toggle(&mut self.selected_skills, &skill.manifest.id);
                            }
                        }
                    });
                }
            });
            ui.add_space(4.0);
            theme::panel_frame().show(ui, |right| {
                right.set_width(right_width);
                let selected = self
                    .selected_skills
                    .iter()
                    .next()
                    .and_then(|id| self.skills.iter().find(|skill| skill.manifest.id == *id))
                    .cloned();
                section_header(right, "详情", "查看选中 skill 的同步信息");
                if let Some(skill) = selected {
                    detail_title(right, &skill.manifest.name, &skill.manifest.id);
                    detail_row(right, "版本", &skill.manifest.version);
                    detail_row(right, "支持", &skill.manifest.supported_agents.join(", "));
                    detail_row(right, "源目录", &skill.source_path);
                    detail_row(right, "Manifest", &skill.manifest_path);
                    right.add_space(8.0);
                    pill(
                        right,
                        &format!("已选择 {} 个 skills", self.selected_skills.len()),
                        theme::ACCENT_SOFT,
                        theme::ACCENT,
                    );
                } else {
                    empty_state(right, "未选择 skill", "在左侧列表中选择一个或多个 skill。");
                }
            });
        });
    }

    fn agents_view(&mut self, ui: &mut Ui) {
        let (left_width, right_width) = content_widths(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_height(ui.available_height());
            theme::panel_frame().show(ui, |left| {
                left.set_width(left_width);
                section_header(left, "Agents", "选择要安装 skills 的目标");
                let agents = self.agents.clone();
                ScrollArea::vertical().show(left, |ui| {
                    if agents.is_empty() {
                        empty_state(ui, "没有发现 agent。", "可以在右侧添加自定义 agent。");
                    }
                    for agent in agents {
                        let selected = self.selected_agents.contains(&agent.id);
                        if agent_card(ui, &agent, selected).clicked() {
                            toggle(&mut self.selected_agents, &agent.id);
                        }
                    }
                });
            });
            ui.add_space(4.0);
            theme::panel_frame().show(ui, |right| {
                right.set_width(right_width);
                section_header(right, "自定义 Agent", "添加一个普通目录作为同步目标");
                label_input(right, "名称", &mut self.custom_agent_name, "例如 My Agent");
                right.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Skills 安装目录").color(theme::MUTED));
                        ui.add_sized(
                            [ui.available_width().max(220.0), 34.0],
                            TextEdit::singleline(&mut self.custom_agent_path).hint_text("选择或输入目录"),
                        );
                    });
                    if secondary_button(ui, "选择").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.custom_agent_path = path.to_string_lossy().to_string();
                        }
                    }
                });
                if primary_button(right, "添加 Agent").clicked() {
                    self.add_custom_agent();
                }
                right.add_space(16.0);
                pill(
                    right,
                    &format!("已选择 {} 个 agents", self.selected_agents.len()),
                    theme::ACCENT_SOFT,
                    theme::ACCENT,
                );
            });
        });
    }

    fn sync_view(&mut self, ui: &mut Ui) {
        let (left_width, right_width) = content_widths(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_height(ui.available_height());
            theme::panel_frame().show(ui, |left| {
                left.set_width(left_width);
                section_header(left, "同步矩阵", "检查每个 skill 到每个 agent 的状态");
                left.horizontal_wrapped(|ui| {
                    policy_button(ui, &mut self.conflict_policy, ConflictPolicy::Prompt, "提示冲突");
                    policy_button(
                        ui,
                        &mut self.conflict_policy,
                        ConflictPolicy::BackupOverwrite,
                        "备份覆盖",
                    );
                    policy_button(ui, &mut self.conflict_policy, ConflictPolicy::Skip, "跳过冲突");
                    policy_button(ui, &mut self.conflict_policy, ConflictPolicy::Rename, "另存副本");
                });
                left.add_space(8.0);
                let states = self.state_by_pair();
                ScrollArea::vertical().show(left, |ui| {
                    if self.selected_skills.is_empty() || self.selected_agents.is_empty() {
                        empty_state(ui, "还没有同步矩阵", "请先在 Skills 和 Agents 页面选择项目。");
                    }
                    for skill_id in &self.selected_skills {
                        for agent_id in &self.selected_agents {
                            let key = format!("{}:{}", agent_id, skill_id);
                            let state = states.get(&key).copied();
                            sync_row(ui, skill_id, &agent_name(&self.agents, agent_id), state);
                        }
                    }
                });
            });
            ui.add_space(4.0);
            theme::panel_frame().show(ui, |right| {
                right.set_width(right_width);
                section_header(right, "执行", "同步选中的组合");
                detail_row(right, "Skills", &self.selected_skills.len().to_string());
                detail_row(right, "Agents", &self.selected_agents.len().to_string());
                detail_row(right, "策略", conflict_policy_label(&self.conflict_policy));
                if primary_button(right, "执行同步").clicked() {
                    self.install_selected();
                }
                right.add_space(16.0);
                section_header(right, "最近结果", "");
                ScrollArea::vertical().max_height(280.0).show(right, |ui| {
                    if self.results.is_empty() {
                        empty_state(ui, "暂无同步结果", "执行同步后会显示安装、更新或跳过记录。");
                    }
                    for result in &self.results {
                        result_card(ui, result);
                    }
                });
            });
        });
    }

    fn settings_view(&mut self, ui: &mut Ui) {
        let (left_width, right_width) = content_widths(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_height(ui.available_height());
            theme::panel_frame().show(ui, |left| {
                left.set_width(left_width);
                section_header(left, "路径", "检查应用使用的本地目录");
                label_input(left, "主仓库", &mut self.repository, "Skills 主仓库路径");
                left.horizontal(|ui| {
                    if secondary_button(ui, "选择目录").clicked() {
                        self.choose_repository();
                    }
                    if primary_button(ui, "保存主仓库").clicked() {
                        self.save_repository();
                    }
                });
                left.add_space(12.0);
                path_row(left, "应用数据", &self.service.data_dir().to_string_lossy());
                path_row(left, "备份目录", &self.service.backup_root().to_string_lossy());
                path_row(left, "导入缓存", &self.service.import_root().to_string_lossy());
            });
            ui.add_space(4.0);
            theme::panel_frame().show(ui, |right| {
                right.set_width(right_width);
                section_header(right, "关于", "当前运行的是原生桌面版");
                detail_row(right, "UI", "egui / eframe");
                detail_row(right, "运行方式", "不依赖浏览器或 localhost");
                detail_row(right, "版本", "0.1.0");
                action_panel(right, "便携版", "使用 scripts/build-portable.ps1 生成 Windows 便携版目录。");
            });
        });
    }
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Skills Manager")
            .with_inner_size(Vec2::new(1280.0, 820.0))
            .with_min_inner_size(Vec2::new(960.0, 680.0)),
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

fn nav_button(ui: &mut Ui, view: &mut View, target: View, label: &str) {
    let selected = *view == target;
    let response = ui.add_sized(
        [ui.available_width(), 36.0],
        Button::new(RichText::new(label).color(if selected {
            theme::ACCENT
        } else {
            theme::TEXT
        }))
        .fill(if selected { theme::ACCENT_SOFT } else { theme::PANEL })
        .stroke(Stroke::new(1.0, if selected { theme::ACCENT } else { theme::BORDER }))
        .rounding(Rounding::same(9.0)),
    );
    if response.clicked() {
        *view = target;
    }
}

fn content_widths(total: f32) -> (f32, f32) {
    let gap = 12.0;
    let usable = (total - gap).max(640.0);
    let right = usable.clamp(320.0, 390.0);
    let left = (usable - right).max(360.0);
    (left, right)
}

fn primary_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).color(Color32::WHITE))
            .fill(theme::ACCENT)
            .stroke(Stroke::new(1.0, theme::ACCENT))
            .rounding(Rounding::same(8.0)),
    )
}

fn secondary_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).color(theme::TEXT))
            .fill(theme::PANEL)
            .stroke(Stroke::new(1.0, theme::BORDER_STRONG))
            .rounding(Rounding::same(8.0)),
    )
}

fn policy_button(ui: &mut Ui, current: &mut ConflictPolicy, value: ConflictPolicy, label: &str) {
    let selected = *current == value;
    let response = ui.add(
        Button::new(RichText::new(label).color(if selected {
            theme::ACCENT
        } else {
            theme::MUTED
        }))
        .fill(if selected { theme::ACCENT_SOFT } else { theme::PANEL })
        .stroke(Stroke::new(1.0, if selected { theme::ACCENT } else { theme::BORDER }))
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

fn skill_card(ui: &mut Ui, skill: &SkillSummary, selected: bool) -> egui::Response {
    let id = ui.id().with(("skill-card", &skill.manifest.id));
    let inner = theme::list_item_frame(selected, false).show(ui, |ui| {
        ui.set_min_height(62.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(&skill.manifest.name).strong());
                ui.label(
                    RichText::new(
                        skill
                            .manifest
                            .description
                            .as_deref()
                            .unwrap_or(&skill.manifest.id),
                    )
                    .color(theme::MUTED),
                );
                ui.horizontal_wrapped(|ui| {
                    pill(ui, &format!("v{}", skill.manifest.version), theme::PANEL_SOFT, theme::MUTED);
                    pill(
                        ui,
                        &skill.manifest.supported_agents.join(", "),
                        theme::PANEL_SOFT,
                        theme::MUTED,
                    );
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(
                    ui,
                    if selected { "已选择" } else { "选择" },
                    if selected { theme::ACCENT_SOFT } else { theme::PANEL_SOFT },
                    if selected { theme::ACCENT } else { theme::MUTED },
                );
            });
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

fn agent_card(ui: &mut Ui, agent: &AgentProfile, selected: bool) -> egui::Response {
    let id = ui.id().with(("agent-card", &agent.id));
    let inner = theme::list_item_frame(selected, false).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(agent_label(agent)).strong());
                ui.label(RichText::new(&agent.skills_path).color(theme::MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(
                    ui,
                    if selected { "已选择" } else { agent.agent_type.as_str() },
                    if selected { theme::ACCENT_SOFT } else { theme::PANEL_SOFT },
                    if selected { theme::ACCENT } else { theme::MUTED },
                );
            });
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

fn sync_row(ui: &mut Ui, skill_id: &str, agent_name: &str, state: Option<&InstallState>) {
    theme::soft_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(skill_id).strong());
            ui.label(RichText::new(agent_name).color(theme::MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (label, color) = status_label(state.map(|state| &state.status));
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

fn detail_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(20.0).strong());
    ui.label(RichText::new(subtitle).color(theme::MUTED));
    ui.add_space(12.0);
}

fn detail_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.set_min_height(28.0);
        ui.label(RichText::new(label).color(theme::MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).color(theme::TEXT));
        });
    });
}

fn path_row(ui: &mut Ui, label: &str, path: &str) {
    theme::soft_frame().show(ui, |ui| {
        ui.label(RichText::new(label).color(theme::MUTED));
        ui.monospace(path);
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
    Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .rounding(Rounding::same(999.0))
        .inner_margin(Margin::symmetric(9.0, 3.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).size(12.0));
        });
}

fn toggle(set: &mut HashSet<String>, value: &str) {
    if !set.insert(value.to_string()) {
        set.remove(value);
    }
}

fn skill_matches(skill: &SkillSummary, query: &str) -> bool {
    query.is_empty()
        || skill.manifest.name.to_lowercase().contains(query)
        || skill.manifest.id.to_lowercase().contains(query)
}

fn active_query(local: &str, global: &str) -> String {
    if local.trim().is_empty() {
        global.trim().to_lowercase()
    } else {
        local.trim().to_lowercase()
    }
}

fn status_label(status: Option<&InstallStatus>) -> (&'static str, Color32) {
    match status {
        Some(InstallStatus::Installed) => ("已安装", theme::ACCENT),
        Some(InstallStatus::Stale) => ("需更新", theme::WARNING),
        Some(InstallStatus::Conflict) => ("冲突", theme::DANGER),
        Some(InstallStatus::Missing) | None => ("未安装", theme::MUTED),
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
