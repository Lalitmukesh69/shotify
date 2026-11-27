use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::image::Handle as ImageHandle;
use iced::{window, Point, Size};
use image::RgbaImage;
use shotify_annotate::{AnnotationTool, ShapeMode};
use shotify_core::{AspectMode, BlobGradientConfig, CompositionTree, GradientPreset};
use shotify_export::{ExportFormat, ExportOutcome};

use crate::config::{Language, WallpaperTab};
use crate::platform::updater::UpdateCheckStatus;
use crate::platform::PlatformCommand;
use crate::theme::ThemeMode;

/// Output of an asynchronous scene composition task.
#[derive(Debug, Clone)]
pub struct RenderedScenePayload {
  pub generation: u64,
  pub image: RgbaImage,
  pub tree: CompositionTree,
}

/// Interactive actions for custom shadow sliders.
#[derive(Debug, Clone)]
pub enum ShadowFieldUpdate {
  OffsetX(f64),
  OffsetY(f64),
  Blur(f64),
  Spread(f64),
  Opacity(f64),
}

/// Targets that open the reusable floating HSV + Hex color picker popover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPickerTarget {
  GradientStart,
  GradientEnd,
  SolidColor,
  BorderColor,
  CustomShadowColor,
  AnnotationColor,
}

/// Interactive toggles in the Settings modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralToggleKey {
  HideAppWhenUnfocused,
  KeepAppInTray,
  HideWindowOnLaunch,
  OpenAtLogin,
  HideDockIconWhenHidden,
  CloseAfterCopy,
  CloseAfterSave,
  CloseAfterSaveToFolder,
}

/// Export destination triggered from the footer or keyboard shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportActionKind {
  Clipboard,
  SaveDownloadsOrDialog,
  SaveToFolder,
}

/// Interactive shortcut slots configurable in Settings -> Shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutFieldKey {
  Copy,
  Save,
  SaveToFolder,
  ScreenshotRegion,
  ScreenshotRegionCopy,
}

/// Top-level message enum driving the Elm/Iced architecture of `shotify-rust`.
#[derive(Debug, Clone)]
pub enum Message {
  // Application lifecycle & system integration events
  InitializePlatformServices,
  WindowOpened(window::Id),
  WindowResized(window::Id, Size),
  WindowMaximizedQueried(bool),
  WindowTitleBarPressed,
  WindowMinimize,
  WindowToggleMaximize,
  WindowCloseButton,
  WindowCloseRequested(window::Id),
  WindowFocused(window::Id),
  WindowUnfocused(window::Id),
  WindowFileDropped(PathBuf),
  PlatformCommandReceived(PlatformCommand),
  KeyboardEvent(iced::keyboard::Event),
  TickClearToast(u64),
  DismissToast(u64),

  // Image loading & desktop capture
  OpenImageDialogRequested,
  ImageFilePicked(Option<PathBuf>),
  PasteFromClipboardRequested,
  StartRegionCaptureRequested,
  ScreenCapturedForOverlay(Result<RgbaImage, String>),
  CaptureDragStart(Point),
  CaptureDragMove(Point),
  CaptureDragEnd(Point),
  CancelCaptureOverlay,
  ClearCanvas,

  // Background thumbnail cache updates & popover triggers
  GradientThumbsGenerated(HashMap<String, ImageHandle>),
  AutoThumbsGenerated(HashMap<u32, ImageHandle>),
  SystemWallpaperThumbsLoaded(HashMap<PathBuf, ImageHandle>),
  OpenColorPicker(ColorPickerTarget),
  CloseColorPicker,
  ColorPickerHsvChanged { h: f32, s: f32, v: f32 },
  ColorPickerHexChanged(String),

  // Wallpaper & background controls
  SelectWallpaperTab(WallpaperTab),
  PickCustomWallpaperRequested,
  CustomWallpaperPicked(Option<PathBuf>),
  SelectSystemWallpaper(PathBuf),
  WallpaperLoaded(PathBuf, Result<RgbaImage, String>),
  SelectGradientPreset(GradientPreset),
  SelectBlobPreset(BlobGradientConfig),
  SelectSolidColor(String),
  GradientStartChanged(String),
  GradientEndChanged(String),
  GradientAngleChanged(f64),
  GradientAngleInputChanged(String),

  // Card layout & styling controls
  ToggleRatioMenu,
  AspectModeSelected(AspectMode, String),
  CustomRatioWChanged(String),
  CustomRatioHChanged(String),
  PaddingChanged(f64),
  InsetChanged(f64),
  CurveChanged(u32),
  BorderWidthChanged(f64),
  AutoCropToggled(bool),
  SelectShadowStyle(String),
  UpdateCustomShadow(ShadowFieldUpdate),
  SelectMockup(String),
  SafariUrlChanged(String),
  UiScaleChanged(f64),

  // Watermark & 3D position controls
  WatermarkEnabledToggled(bool),
  WatermarkPositionSelected(String),
  WatermarkTextChanged(String),
  WatermarkPrefixSelected(String),
  WatermarkVerifiedToggled(bool),
  WatermarkOffsetXInputChanged(String),
  WatermarkOffsetYInputChanged(String),
  ToggleMorePosition,
  TiltXChanged(f64),
  TiltYChanged(f64),
  RotationChanged(f64),
  CardPositionXInputChanged(String),
  CardPositionYInputChanged(String),
  CardDraggedDelta(f64, f64),
  SliderReleased,
  NoOp,

  // Async composition result
  CompositionFinished(Result<RenderedScenePayload, String>),

  // Annotation toolbar & interactive canvas
  SelectAnnotationTool(Option<AnnotationTool>),
  SetShapeMode(ShapeMode),
  AnnotationPointerDown { px: f64, py: f64, doc_w: f64, doc_h: f64 },
  AnnotationPointerMove { px: f64, py: f64, doc_w: f64, doc_h: f64 },
  AnnotationPointerUp,
  AnnotationDoubleClick { px: f64, py: f64, doc_w: f64, doc_h: f64 },
  TextPromptChanged(String),
  TextPromptCommit,
  TextPromptCancel,
  AnnotationUndo,
  AnnotationRedo,
  AnnotationClearAll,

  // Presets
  PresetDropdownSelected(String),
  OpenRenamePresetDialog(String),
  CloseSavePresetDialog,
  PresetNameDraftChanged(String),
  ConfirmSavePreset,
  RequestDeletePreset(String),
  CancelDeletePreset,
  DeletePreset(String),

  // Export footer
  ToggleExportDrawer,
  CloseExportDrawer,
  SelectExportScale(f64),
  SelectExportFormat(ExportFormat),
  ExportQualityChanged(f64),
  RunExport(ExportActionKind),
  SaveFilePicked(Option<PathBuf>),
  SaveToFolderPicked(Option<PathBuf>),
  ExportFinished(ExportActionKind, Result<ExportOutcomeSummary, String>),
  PickSaveFolderRequested,
  SaveFolderPicked(Option<PathBuf>),

  // Settings modal & Phase 5 integrations
  OpenSettings,
  CloseSettings,
  SelectThemeMode(ThemeMode),
  SelectLanguage(Language),
  ToggleGeneralSetting(GeneralToggleKey, bool),
  StartRecordingShortcut(ShortcutFieldKey),
  ResetShortcutField(ShortcutFieldKey),
  CancelRecordingShortcut,
  ResetAllShortcuts,
  ImportLegacyTauriRequested,
  CheckForUpdatesRequested,
  UpdateCheckFinished(UpdateCheckStatus),
  OpenExternalUrl(String),
}

/// Owned summary returned by background export tasks (`ExportOutcome` without raw buffers).
#[derive(Debug, Clone)]
pub struct ExportOutcomeSummary {
  pub width: u32,
  pub height: u32,
  pub file_path: Option<PathBuf>,
  pub bytes_len: usize,
  pub format: ExportFormat,
}

impl From<ExportOutcome> for ExportOutcomeSummary {
  fn from(value: ExportOutcome) -> Self {
    Self {
      width: value.width,
      height: value.height,
      file_path: value.file_path,
      bytes_len: value.bytes.len(),
      format: value.format,
    }
  }
}
