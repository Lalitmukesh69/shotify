import { useCallback, useEffect, useRef, useState } from 'react'
import { installTauriApi } from '../../../utils/platform/tauriApi'
import { trimMacOsWindowScreenshotDataUrl } from '../../../utils/export/imageProcessor'
import { copyImage as copyImageUtil, createExportableSnapshot, saveImageAdvanced, scaleAndEncodeBlob } from '../../../utils/export/imageProcessor'
import { readFileAsDataUrlWithRetry } from '../../../utils/export/readFileAsDataUrlWithRetry'
import { getJson, setJson } from '../../../utils/platform/safeStorage'
import { formatExportErrorMessage, getDesktopApi, getNativeInvoke } from '../AppHelpers'

const fileToDataUrl = (file) => new Promise((resolve, reject) => {
  const reader = new FileReader()
  reader.onerror = () => reject(new Error('Failed to read image file'))
  reader.onload = () => resolve(String(reader.result || ''))
  reader.readAsDataURL(file)
})

const isImageFileLike = (file) => {
  if (!file) return false
  if (String(file.type || '').toLowerCase().startsWith('image/')) return true
  return /\.(png|jpe?g|webp|bmp|gif|svg|avif|heic|heif)$/i.test(String(file.name || ''))
}

const readClipboardEventImageFile = (clipboardEvent) => {
  const dt = clipboardEvent?.clipboardData || clipboardEvent?.originalEvent?.clipboardData || clipboardEvent?.dataTransfer
  if (!dt) return null
  const items = dt.items
  if (items && typeof items.length === 'number') {
    for (let i = 0; i < items.length; i += 1) {
      const item = items[i]
      if (item?.kind === 'file' || String(item?.type || '').toLowerCase().startsWith('image/')) {
        const file = typeof item.getAsFile === 'function' ? item.getAsFile() : item
        if (isImageFileLike(file)) return file
      }
    }
  }
  const files = dt.files
  if (files && typeof files.length === 'number') {
    for (let i = 0; i < files.length; i += 1) {
      if (isImageFileLike(files[i])) return files[i]
    }
  }
  return null
}

/**
 * Provides export functions and related state.
 */
export function useExportFunctions({
  blob,
  imageOptions,
  exportScale,
  exportFormat,
  exportQuality,
  exportRef,
  appWindow,
  generalSettings,
  pushToast,
  setBlob,
  setTransform,
  setAnnotationTool,
  rootFocusRef,
}) {
  const [exportFolderPath, setExportFolderPath] = useState(() => {
    try {
      return getJson('uiExp.exportFolderPath', null)
    } catch {
      return null
    }
  })
  const [exportBusy, setExportBusy] = useState(false)
  const [exportDrawerOpen, setExportDrawerOpen] = useState(false)
  const exportDrawerRef = useRef(null)

  // Close export drawer on outside click
  useEffect(() => {
    if (!exportDrawerOpen) return
    const handler = (e) => {
      if (exportDrawerRef.current && !exportDrawerRef.current.contains(e.target)) {
        setExportDrawerOpen(false)
      }
    }
    document.addEventListener('pointerdown', handler, true)
    return () => document.removeEventListener('pointerdown', handler, true)
  }, [exportDrawerOpen])

  const pickExportFolder = useCallback(async () => {
    const api = window?.tauriAPI
    if (!api?.pickDirectory) return null
    let picked = null
    try {
      picked = await api.pickDirectory({ title: 'Choose export folder' })
    } catch (e) {
      pushToast(`Failed to pick folder: ${e?.message || String(e)}`, { variant: 'error', durationMs: 3500 })
      return null
    }
    if (picked) {
      try { setJson('uiExp.exportFolderPath', picked) } catch {}
      setExportFolderPath(picked)
      pushToast('Export folder set', { variant: 'success' })
      return picked
    }
    return null
  }, [pushToast])

  const buildDefaultExportFileName = useCallback(() => {
    const ts = new Date()
    const pad = (n) => String(n).padStart(2, '0')
    const stamp = `${ts.getFullYear()}-${pad(ts.getMonth() + 1)}-${pad(ts.getDate())}_${pad(ts.getHours())}-${pad(ts.getMinutes())}-${pad(ts.getSeconds())}`
    const ext = exportFormat === 'jpeg' ? 'jpg' : exportFormat
    return `shotstyle-${stamp}.${ext}`
  }, [exportFormat])

  const waitForAnimationFrames = useCallback(async (count = 2) => {
    const frames = Math.max(1, Math.floor(Number(count) || 1))
    for (let index = 0; index < frames; index += 1) {
      await new Promise((resolve) => {
        requestAnimationFrame(() => resolve())
      })
    }
  }, [])

  const getExportElement = useCallback(() => exportRef?.current || null, [exportRef])

  const runWithFrozenPreview = useCallback(async (work) => {
    if (exportBusy) return
    setExportBusy(true)
    try {
      await waitForAnimationFrames(2)
      await work()
    } finally {
      window.setTimeout(() => {
        setExportBusy(false)
      }, 120)
    }
  }, [exportBusy, waitForAnimationFrames])

  const saveToFolder = useCallback(async () => {
    await runWithFrozenPreview(async () => {
      try {
        if (!blob?.src) {
          pushToast('Nothing to export yet', { variant: 'error' })
          return
        }

        const api = window?.tauriAPI
        const folder = exportFolderPath || (await pickExportFolder())
        if (!folder) return

        const exportEl = getExportElement()
        if (!exportEl) return

        const snapshotBlob = await createExportableSnapshot(exportEl, { editorOptions: imageOptions })

        let finalBlob = snapshotBlob
        if (exportScale !== 1 || exportFormat !== 'png') {
          try {
            finalBlob = await scaleAndEncodeBlob(snapshotBlob, { scale: exportScale, format: exportFormat, quality: exportQuality })
          } catch {}
        }

        const dataUrl = await new Promise((resolve, reject) => {
          try {
            const r = new FileReader()
            r.onerror = () => reject(new Error('Failed to read export blob'))
            r.onload = () => resolve(String(r.result || ''))
            r.readAsDataURL(finalBlob)
          } catch (e) {
            reject(e)
          }
        })

        const fileName = buildDefaultExportFileName()
        if (api?.saveExportedImageToFolder) {
          await api.saveExportedImageToFolder({ dataUrl, folderPath: folder, fileName })
          pushToast(`Saved: ${fileName}`, { variant: 'success' })
          if (generalSettings?.closeAfterSaveToFolder) {
            try { await (getDesktopApi()?.hideMainWindow?.() || appWindow?.hide?.()) } catch {}
          }
          return
        }

        if (api?.saveExportedImage) {
          await api.saveExportedImage({ dataUrl, fileName })
          pushToast(`Saved: ${fileName}`, { variant: 'success' })
          if (generalSettings?.closeAfterSaveToFolder) {
            try { await (getDesktopApi()?.hideMainWindow?.() || appWindow?.hide?.()) } catch {}
          }
          return
        }

        pushToast('Save not available in this environment', { variant: 'error', durationMs: 3500 })
      } catch (e) {
        pushToast(formatExportErrorMessage('Save', e), { variant: 'error', durationMs: 4500 })
      }
    })
  }, [appWindow, blob?.src, buildDefaultExportFileName, exportFolderPath, exportFormat, exportQuality, exportScale, generalSettings?.closeAfterSaveToFolder, imageOptions, pickExportFolder, pushToast, runWithFrozenPreview])

  const copyOutput = useCallback(async () => {
    await runWithFrozenPreview(async () => {
      try {
        if (!blob?.src) {
          pushToast('Nothing to copy yet', { variant: 'error' })
          return
        }
        const exportEl = getExportElement()
        if (!exportEl) {
          pushToast('Nothing to copy yet', { variant: 'error' })
          return
        }
        await copyImageUtil({ current: exportEl }, blob, imageOptions)
        pushToast('Copied to clipboard', { variant: 'success' })
        if (generalSettings?.closeAfterCopy) {
          try { await (getDesktopApi()?.hideMainWindow?.() || appWindow?.hide?.()) } catch {}
        }
      } catch (e) {
        pushToast(formatExportErrorMessage('Copy', e), { variant: 'error', durationMs: 4500 })
      }
    })
  }, [appWindow, blob, generalSettings?.closeAfterCopy, imageOptions, pushToast, runWithFrozenPreview])

  const saveOutput = useCallback(async () => {
    await runWithFrozenPreview(async () => {
      try {
        if (!blob?.src) {
          pushToast('Nothing to save yet', { variant: 'error' })
          return
        }
        const exportEl = getExportElement()
        if (!exportEl) {
          pushToast('Nothing to save yet', { variant: 'error' })
          return
        }
        const fileName = buildDefaultExportFileName()
        await saveImageAdvanced({ current: exportEl }, blob, {
          fileName,
          scale: exportScale,
          format: exportFormat,
          quality: exportQuality,
        }, imageOptions)
        pushToast(`Saved: ${fileName}`, { variant: 'success' })
        if (generalSettings?.closeAfterSave) {
          try { await (getDesktopApi()?.hideMainWindow?.() || appWindow?.hide?.()) } catch {}
        }
      } catch (e) {
        pushToast(formatExportErrorMessage('Save', e), { variant: 'error', durationMs: 4500 })
      }
    })
  }, [appWindow, blob, buildDefaultExportFileName, exportFormat, exportQuality, exportScale, generalSettings?.closeAfterSave, imageOptions, pushToast, runWithFrozenPreview])

  const applyCapturedSource = useCallback(async (src, { shouldFocus = true } = {}) => {
    if (!src || typeof src !== 'string') return
    const isMac = typeof window !== 'undefined' && !!(window?.platform?.isMac ?? /Mac|iPhone|iPad|iPod/i.test(navigator?.platform || ''))
    const cleaned = (isMac && src.startsWith('data:image/'))
      ? await trimMacOsWindowScreenshotDataUrl(src)
      : src

    setBlob({ src: cleaned, w: 0, h: 0 })
    setTransform({ x: 0, y: 0 })
    setAnnotationTool(null)

    try {
      const img = new Image()
      img.onload = () => {
        const w = Number(img.naturalWidth) || 0
        const h = Number(img.naturalHeight) || 0
        setBlob((prev) => (prev?.src === cleaned ? { ...prev, w, h } : prev))
      }
      img.src = cleaned
    } catch {}

    if (shouldFocus) {
      try { rootFocusRef.current?.focus?.({ preventScroll: true }) } catch {}
    }
  }, [rootFocusRef, setAnnotationTool, setBlob, setTransform])

  const pasteOutput = useCallback(async (clipboardEvent = null) => {
    try {
      const eventFile = readClipboardEventImageFile(clipboardEvent)
      if (eventFile) {
        const dataUrl = await fileToDataUrl(eventFile)
        if (dataUrl.startsWith('data:image/')) {
          await applyCapturedSource(dataUrl, { shouldFocus: true })
          pushToast('Pasted image from clipboard', { variant: 'success' })
          return true
        }
      }

      await installTauriApi().catch(() => {})
      const api = window?.tauriAPI
      const nativeInvoke = getNativeInvoke()
      let nativeDataUrl = null
      if (typeof api?.readClipboardImage === 'function') {
        nativeDataUrl = await api.readClipboardImage()
      } else if (nativeInvoke) {
        nativeDataUrl = await nativeInvoke('clipboard_read_image_data_url')
      }

      if (typeof nativeDataUrl === 'string' && nativeDataUrl.startsWith('data:image/')) {
        await applyCapturedSource(nativeDataUrl, { shouldFocus: true })
        pushToast('Pasted image from clipboard', { variant: 'success' })
        return true
      }

      if (typeof navigator !== 'undefined' && typeof navigator.clipboard?.read === 'function') {
        const items = await navigator.clipboard.read()
        for (const item of items) {
          const imgType = (item?.types || []).find((t) => String(t).toLowerCase().startsWith('image/'))
          if (imgType) {
            const blobItem = await item.getType(imgType)
            const dataUrl = await fileToDataUrl(blobItem)
            if (dataUrl.startsWith('data:image/')) {
              await applyCapturedSource(dataUrl, { shouldFocus: true })
              pushToast('Pasted image from clipboard', { variant: 'success' })
              return true
            }
          }
        }
      }

      pushToast('No image found in clipboard', { variant: 'error', durationMs: 2600 })
      return false
    } catch (e) {
      pushToast(`Paste failed: ${e?.message || String(e)}`, { variant: 'error', durationMs: 3600 })
      return false
    }
  }, [applyCapturedSource, pushToast])

  const takeScreenshot = useCallback(async () => {
    try {
      await installTauriApi()
      const api = window?.tauriAPI
      if (!api?.captureScreenshotToFile) {
        pushToast('Take screenshot is available in the desktop app', { variant: 'error', durationMs: 3600 })
        return
      }

      const result = await api.captureScreenshotToFile({ mode: 'region' })
      if (typeof result !== 'string' || !result) return
      if (result === 'overlay-opened') return

      if (result.startsWith('data:image/')) {
        await applyCapturedSource(result, { shouldFocus: true })
        return
      }

      if (typeof api.readFileAsDataUrl === 'function') {
        const dataUrl = await readFileAsDataUrlWithRetry(api.readFileAsDataUrl, result)
        if (typeof dataUrl === 'string' && dataUrl.startsWith('data:image/')) {
          await applyCapturedSource(dataUrl, { shouldFocus: true })
        }
      }
    } catch (e) {
      pushToast(`Take screenshot failed: ${e?.message || String(e)}`, { variant: 'error', durationMs: 4200 })
    }
  }, [applyCapturedSource, pushToast])

  return {
    exportFolderPath,
    exportBusy,
    exportDrawerOpen,
    setExportDrawerOpen,
    exportDrawerRef,
    pickExportFolder,
    buildDefaultExportFileName,
    waitForAnimationFrames,
    getExportElement,
    runWithFrozenPreview,
    saveToFolder,
    copyOutput,
    saveOutput,
    pasteOutput,
    applyCapturedSource,
    takeScreenshot,
  }
}
