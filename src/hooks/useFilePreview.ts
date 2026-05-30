import { useState, useCallback, useRef } from "react";
import { describeFilePreview, resolveFilePreview } from "../api";
import type { PreviewMode, PreviewState, PreviewTarget } from "../types";

export interface FilePreviewHook {
  state: PreviewState;
  openPreview: (target: PreviewTarget, workspaceRoot: string) => Promise<void>;
  switchMode: (mode: PreviewMode, workspaceRoot: string) => Promise<void>;
  closePreview: () => void;
  openFocused: () => void;
  closeFocused: () => void;
}

const INITIAL: PreviewState = {
  isOpen: false, target: null, descriptor: null, selectedMode: null,
  isLoading: false, error: null, focusedView: false,
};

export function useFilePreview(): FilePreviewHook {
  const [state, setState] = useState<PreviewState>(INITIAL);
  const targetRef = useRef<PreviewTarget | null>(null);

  const openPreview = useCallback(async (target: PreviewTarget, workspaceRoot: string) => {
    targetRef.current = target;
    setState((s) => ({ ...s, isOpen: true, target, isLoading: true, error: null }));
    try {
      const skeleton = await describeFilePreview(target.path, workspaceRoot);
      setState((s) => ({ ...s, descriptor: skeleton, selectedMode: skeleton.default_mode }));
      const full = await resolveFilePreview(target.path, workspaceRoot, skeleton.default_mode);
      setState((s) => ({ ...s, descriptor: full, isLoading: false }));
    } catch (err) {
      setState((s) => ({
        ...s, isLoading: false,
        error: err instanceof Error ? err.message : "预览加载失败",
      }));
    }
  }, []);

  const switchMode = useCallback(async (mode: PreviewMode, workspaceRoot: string) => {
    const target = targetRef.current;
    if (!target) return;
    setState((s) => ({ ...s, selectedMode: mode, isLoading: true, error: null }));
    try {
      const full = await resolveFilePreview(target.path, workspaceRoot, mode);
      setState((s) => ({ ...s, descriptor: full, isLoading: false }));
    } catch (err) {
      setState((s) => ({
        ...s, isLoading: false,
        error: err instanceof Error ? err.message : "模式切换失败",
      }));
    }
  }, []);

  const closePreview = useCallback(() => {
    targetRef.current = null;
    setState(INITIAL);
  }, []);

  const openFocused = useCallback(() => setState((s) => ({ ...s, focusedView: true })), []);
  const closeFocused = useCallback(() => setState((s) => ({ ...s, focusedView: false })), []);

  return { state, openPreview, switchMode, closePreview, openFocused, closeFocused };
}
