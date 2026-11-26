import React, { useCallback, useEffect, useState } from 'react'
import ReactDOM from 'react-dom/client'
import './index.css'
import './App.css'
import '../../styles/globals.css'

import RegionCaptureOverlay from '../../components/overlays/RegionCaptureOverlay.js'
import { installTauriApi } from '../../utils/platform/tauriApi'
import { readFileAsDataUrlWithRetry } from '../../utils/export/readFileAsDataUrlWithRetry'

function getInitialCaptureParams() {
  try {
    const params = new URLSearchParams(window.location.search || '')
    return {
      intent: params.get('intent') || null,
      filePath: params.get('filePath') || null,
      nonce: 'initial',
    }
  } catch {
    return { intent: null, filePath: null, nonce: 'initial' }
  }
}

function CaptureApp() {
  const [captureState, setCaptureState] = useState(getInitialCaptureParams)
  const [imageAssetUrl, setImageAssetUrl] = useState('')

  useEffect(() => {
    let active = true
    let unlisten = null

    ;(async () => {
      try {
        await installTauriApi()
      } catch {}
      try {
        const listen =
          window?.__TAURI__?.event?.listen ||
          window?.__TAURI_INTERNALS__?.event?.listen ||
          (await import('@tauri-apps/api/event'))?.listen

        if (typeof listen === 'function') {
          const fn = await listen('capture-reset', (event) => {
            if (!active) return
            const p = event?.payload || {}
            setCaptureState({
              intent: p.intent || null,
              filePath: p.filePath ?? p.file_path ?? null,
              nonce: p.nonce || `${Date.now()}`,
            })
          })
          if (active) unlisten = fn
          else fn?.()
        }
      } catch {}
    })()

    return () => {
      active = false
      try { unlisten?.() } catch {}
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    const filePath = captureState.filePath
    if (!filePath) {
      setImageAssetUrl('')
      return undefined
    }

    ;(async () => {
      try {
        const api = await installTauriApi()
        if (!cancelled && api && typeof api.readFileAsDataUrl === 'function') {
          try {
            const dataUrl = await readFileAsDataUrlWithRetry(api.readFileAsDataUrl, filePath)
            if (!cancelled && dataUrl) {
              setImageAssetUrl(dataUrl)
              return
            }
          } catch {}
        }
        if (!cancelled && api && typeof api.filePathToAssetUrl === 'function') {
          const assetUrl = api.filePathToAssetUrl(filePath)
          if (assetUrl) setImageAssetUrl(assetUrl)
        }
      } catch {}
    })()

    return () => {
      cancelled = true
    }
  }, [captureState.filePath, captureState.nonce])

  const handleCancel = useCallback(async () => {
    try {
      const api = await installTauriApi()
      await api?.captureCancel?.({ filePath: captureState.filePath || null })
    } catch {}
  }, [captureState.filePath])

  const handleConfirm = useCallback(
    async ({ x, y, w, h }) => {
      if (!captureState.filePath) return
      try {
        const api = await installTauriApi()
        await api?.captureRegionUpload?.({
          filePath: captureState.filePath,
          x,
          y,
          w,
          h,
          dpr: typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1,
          intent: captureState.intent || null,
        })
      } catch {}
    },
    [captureState.filePath, captureState.intent]
  )

  return (
    <RegionCaptureOverlay
      key={captureState.nonce}
      isOpen
      imageAssetUrl={imageAssetUrl}
      onCancel={handleCancel}
      onConfirm={handleConfirm}
    />
  )
}

if (typeof window !== 'undefined') {
  installTauriApi().catch(() => {})
}

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <CaptureApp />
  </React.StrictMode>
)
