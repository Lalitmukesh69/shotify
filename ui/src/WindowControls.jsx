import React from 'react'
import { Minus, Square, X } from 'lucide-react'
import { useAppContext } from './AppContext'

export function LinuxTitleBar() {
  const { showSlimLinuxTitleBar, appWindow } = useAppContext()

  if (!showSlimLinuxTitleBar || !appWindow) return null

  return (
    <div
      data-tauri-drag-region
      className="h-[24px] w-full shrink-0 bg-[#050507] border-b border-white/[0.06] px-2.5 flex items-center justify-between select-none z-50"
      aria-label="Window title bar"
      onMouseDown={(e) => {
        if (e.button !== 0) return
        if (e.target instanceof Element && e.target.closest('button')) return
        try { void appWindow.startDragging?.() } catch {}
      }}
      onDoubleClick={(e) => {
        if (e.target instanceof Element && e.target.closest('button')) return
        try { void appWindow.toggleMaximize?.() } catch {}
      }}
    >
      <span className="text-[11px] font-inter font-medium tracking-wide text-white/45 pointer-events-none select-none">
        shotify
      </span>

      <div className="flex items-center gap-0.5 window-no-drag" aria-label="Window controls">
        <button
          type="button"
          className="h-4 w-6 rounded-[4px] text-white/55 hover:text-white hover:bg-white/10 transition-colors grid place-items-center"
          aria-label="Minimize"
          title="Minimize"
          onClick={async () => {
            try { await appWindow.minimize?.() } catch {}
          }}
        >
          <Minus size={10} strokeWidth={2} />
        </button>
        <button
          type="button"
          className="h-4 w-6 rounded-[4px] text-white/55 hover:text-white hover:bg-white/10 transition-colors grid place-items-center"
          aria-label="Maximize"
          title="Maximize"
          onClick={async () => {
            try { await appWindow.toggleMaximize?.() } catch {}
          }}
        >
          <Square size={9} strokeWidth={2} />
        </button>
        <button
          type="button"
          className="h-4 w-6 rounded-[4px] text-white/55 hover:text-white hover:bg-[#E81123]/85 transition-colors grid place-items-center"
          aria-label="Close"
          title="Close"
          onClick={async () => {
            try { await appWindow.close?.() } catch {}
          }}
        >
          <X size={11} strokeWidth={2} />
        </button>
      </div>
    </div>
  )
}

const RESIZE_EDGES = [
  { dir: 'North', className: 'top-0 left-2 right-2 h-[3px] cursor-ns-resize' },
  { dir: 'South', className: 'bottom-0 left-2 right-2 h-[3px] cursor-ns-resize' },
  { dir: 'West', className: 'left-0 top-2 bottom-2 w-[3px] cursor-ew-resize' },
  { dir: 'East', className: 'right-0 top-2 bottom-2 w-[3px] cursor-ew-resize' },
  { dir: 'NorthWest', className: 'top-0 left-0 w-2 h-2 cursor-nwse-resize' },
  { dir: 'NorthEast', className: 'top-0 right-0 w-2 h-2 cursor-nesw-resize' },
  { dir: 'SouthWest', className: 'bottom-0 left-0 w-2 h-2 cursor-nesw-resize' },
  { dir: 'SouthEast', className: 'bottom-0 right-0 w-2 h-2 cursor-nwse-resize' },
]

export function LinuxResizeHandles() {
  const { showSlimLinuxTitleBar, appWindow } = useAppContext()

  if (!showSlimLinuxTitleBar || !appWindow) return null

  return (
    <>
      {RESIZE_EDGES.map(({ dir, className }) => (
        <div
          key={dir}
          aria-hidden="true"
          className={`fixed z-[60] window-no-drag ${className}`}
          onMouseDown={(e) => {
            if (e.button !== 0) return
            e.preventDefault()
            try { void appWindow.startResizeDragging?.(dir) } catch {}
          }}
        />
      ))}
    </>
  )
}

export default function WindowControls() {
  const { isTauri, isWindows, showCustomWindowControls, showSlimLinuxTitleBar, appWindow } = useAppContext()
  const shouldRender = isTauri && !!appWindow && !showSlimLinuxTitleBar && (typeof showCustomWindowControls === 'boolean' ? showCustomWindowControls : isWindows)

  if (!shouldRender) return null

  return (
    <div
      className="absolute top-1.5 right-3 z-40 flex items-center gap-1 window-no-drag"
      aria-label="Window controls"
    >
      <button
        type="button"
        className="h-7 w-9 rounded-[8px] text-white/85 hover:bg-white/10 transition-colors grid place-items-center"
        aria-label="Minimize"
        title="Minimize"
        onClick={async () => {
          try { await appWindow.minimize?.() } catch {}
        }}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        className="h-7 w-9 rounded-[8px] text-white/85 hover:bg-white/10 transition-colors grid place-items-center"
        aria-label="Maximize"
        title="Maximize"
        onClick={async () => {
          try { await appWindow.toggleMaximize?.() } catch {}
        }}
      >
        <Square size={12} />
      </button>
      <button
        type="button"
        className="h-7 w-9 rounded-[8px] text-white/85 hover:bg-[#E81123] transition-colors grid place-items-center"
        aria-label="Close"
        title="Close"
        onClick={async () => {
          try { await appWindow.close?.() } catch {}
        }}
      >
        <X size={14} />
      </button>
    </div>
  )
}
