import { useState, useEffect, useCallback, useRef, KeyboardEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import {
  File,
  X,
  Search,
  Replace,
  Save,
  FolderOpen,
  Circle,
} from 'lucide-react';
import clsx from 'clsx';
import type { EditorFile, SearchMatch } from '@/types';

export default function CodeEditor() {
  const { t } = useTranslation();
  const [openFiles, setOpenFiles] = useState<EditorFile[]>([]);
  const [activeFileId, setActiveFileId] = useState<string | null>(null);
  const [content, setContent] = useState('');
  const [showSearch, setShowSearch] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [replaceText, setReplaceText] = useState('');
  const [searchMatches, setSearchMatches] = useState<SearchMatch[]>([]);
  const [useRegex, setUseRegex] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const activeFile = openFiles.find((f) => f.id === activeFileId) || null;

  const loadOpenFiles = useCallback(async () => {
    try {
      const files = await invoke<EditorFile[]>('editor_list_open');
      setOpenFiles(files);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadOpenFiles();
  }, [loadOpenFiles]);

  const handleOpen = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({});
      if (selected) {
        const file = await invoke<EditorFile>('editor_open', { path: selected });
        setOpenFiles((prev) => [...prev, file]);
        setActiveFileId(file.id);
        setContent(file.content);
      }
    } catch {
      // ignore
    }
  };

  const handleSave = async () => {
    if (!activeFileId) return;
    try {
      await invoke('editor_save', { fileId: activeFileId, content });
      setOpenFiles((prev) =>
        prev.map((f) => (f.id === activeFileId ? { ...f, content, modified: false } : f))
      );
    } catch {
      // ignore
    }
  };

  const handleClose = async (fileId: string) => {
    try {
      await invoke('editor_close', { fileId });
      setOpenFiles((prev) => prev.filter((f) => f.id !== fileId));
      if (activeFileId === fileId) {
        const remaining = openFiles.filter((f) => f.id !== fileId);
        if (remaining.length > 0) {
          setActiveFileId(remaining[0].id);
          setContent(remaining[0].content);
        } else {
          setActiveFileId(null);
          setContent('');
        }
      }
    } catch {
      // ignore
    }
  };

  const handleTabClick = (file: EditorFile) => {
    setActiveFileId(file.id);
    setContent(file.content);
  };

  const handleContentChange = (newContent: string) => {
    setContent(newContent);
    setOpenFiles((prev) =>
      prev.map((f) => (f.id === activeFileId ? { ...f, modified: true } : f))
    );
  };

  const handleSearch = async () => {
    if (!activeFileId || !searchQuery) return;
    try {
      const matches = await invoke<SearchMatch[]>('editor_search', {
        fileId: activeFileId,
        query: searchQuery,
        regex: useRegex,
      });
      setSearchMatches(matches);
    } catch {
      setSearchMatches([]);
    }
  };

  const handleReplace = async (all: boolean) => {
    if (!activeFileId || !searchQuery) return;
    try {
      const count = await invoke<number>('editor_replace', {
        fileId: activeFileId,
        query: searchQuery,
        replacement: replaceText,
        regex: useRegex,
        all,
      });
      if (count > 0) {
        const newContent = await invoke<string>('editor_get_content', {
          fileId: activeFileId,
        });
        setContent(newContent);
        handleSearch();
      }
    } catch {
      // ignore
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
      e.preventDefault();
      setShowSearch(true);
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      handleSave();
    }
  };

  const lineNumbers = content.split('\n').length;
  const fileName = (path: string) => path.split('/').pop() || path;

  return (
    <div className="flex flex-col h-full bg-surface-primary" role="application" onKeyDown={handleKeyDown}>
      {/* File Tabs */}
      <div className="flex items-center bg-surface-secondary border-b border-border-default overflow-x-auto">
        {openFiles.map((file) => (
          <div
            key={file.id}
            role="tab"
            tabIndex={0}
            onClick={() => handleTabClick(file)}
            onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleTabClick(file); }}
            className={clsx(
              'flex items-center gap-1 px-3 py-1.5 text-xs cursor-pointer border-r border-border-subtle transition-colors group',
              file.id === activeFileId
                ? 'bg-surface-primary text-text-primary'
                : 'text-text-secondary hover:bg-surface-elevated'
            )}
          >
            <File size={12} />
            <span>{fileName(file.path)}</span>
            {file.modified && (
              <Circle size={6} className="fill-current text-accent-primary" />
            )}
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleClose(file.id);
              }}
              className="ml-1 p-0.5 rounded opacity-0 group-hover:opacity-100 hover:bg-surface-elevated transition-opacity"
            >
              <X size={10} />
            </button>
          </div>
        ))}
        <button
          onClick={handleOpen}
          className="flex items-center gap-1 px-3 py-1.5 text-xs text-text-secondary hover:text-text-primary transition-colors"
          title={t('editor.open')}
        >
          <FolderOpen size={12} />
        </button>
        {activeFile && (
          <button
            onClick={handleSave}
            className="ml-auto flex items-center gap-1 px-2 py-1 mr-2 text-xs text-text-secondary hover:text-text-primary transition-colors"
            title={t('editor.save')}
          >
            <Save size={12} />
          </button>
        )}
      </div>

      {/* Search/Replace Bar */}
      {showSearch && (
        <div className="flex items-center gap-2 px-3 py-2 bg-surface-secondary border-b border-border-default">
          <Search size={14} className="text-text-secondary" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
            placeholder={t('editor.search')}
            className="flex-1 px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
            autoFocus
          />
          <label className="flex items-center gap-1 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={useRegex}
              onChange={(e) => setUseRegex(e.target.checked)}
              className="w-3 h-3"
            />{' '}
            Regex
          </label>
          <Replace size={14} className="text-text-secondary" />
          <input
            type="text"
            value={replaceText}
            onChange={(e) => setReplaceText(e.target.value)}
            placeholder={t('editor.replace')}
            className="w-32 px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
          />
          <button
            onClick={() => handleReplace(false)}
            className="px-2 py-1 text-xs rounded bg-surface-elevated text-text-secondary hover:text-text-primary border border-border-subtle"
          >
            {t('editor.replace')}
          </button>
          <button
            onClick={() => handleReplace(true)}
            className="px-2 py-1 text-xs rounded bg-surface-elevated text-text-secondary hover:text-text-primary border border-border-subtle"
          >
            {t('editor.replaceAll')}
          </button>
          {searchMatches.length > 0 && (
            <span className="text-xs text-text-secondary">
              {searchMatches.length} found
            </span>
          )}
          <button
            onClick={() => {
              setShowSearch(false);
              setSearchMatches([]);
            }}
            className="p-1 text-text-secondary hover:text-text-primary"
          >
            <X size={12} />
          </button>
        </div>
      )}

      {/* Editor Area */}
      {activeFile ? (
        <div className="flex-1 flex overflow-hidden">
          {/* Line Numbers */}
          <div className="flex-shrink-0 w-12 bg-surface-sunken border-r border-border-subtle overflow-hidden text-right pr-2 pt-2 select-none">
            {Array.from({ length: lineNumbers }, (_, i) => (
              <div key={i} className="text-xs text-text-disabled leading-5 h-5">
                {i + 1}
              </div>
            ))}
          </div>
          {/* Text Area */}
          <textarea
            ref={textareaRef}
            value={content}
            onChange={(e) => handleContentChange(e.target.value)}
            className="flex-1 bg-surface-primary text-text-primary font-mono text-sm leading-5 p-2 resize-none outline-none overflow-auto"
            spellCheck={false}
          />
        </div>
      ) : (
        <div className="flex-1 flex flex-col items-center justify-center text-text-secondary gap-2">
          <File size={48} className="opacity-30" />
          <p className="text-sm">{t('editor.noFiles')}</p>
          <button
            onClick={handleOpen}
            className="flex items-center gap-1 px-3 py-1.5 text-sm rounded bg-interactive-default text-text-inverse hover:bg-interactive-hover transition-colors"
          >
            <FolderOpen size={14} />
            {t('editor.open')}
          </button>
        </div>
      )}

      {/* Status Bar */}
      {activeFile && (
        <div className="flex items-center gap-4 px-3 py-1 bg-surface-secondary border-t border-border-default text-xs text-text-secondary">
          <span>{activeFile.language || 'plaintext'}</span>
          <span>{activeFile.encoding}</span>
          <span>{lineNumbers} lines</span>
          {activeFile.modified && <span className="text-accent-primary">Modified</span>}
        </div>
      )}
    </div>
  );
}
