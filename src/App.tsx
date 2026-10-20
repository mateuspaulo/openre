import React, { useState, useEffect, useRef } from 'react';
import {
  HardDrive,
  FileSearch,
  ShieldCheck,
  RotateCcw,
  Settings,
  Info,
  FolderOpen,
  ArrowRight,
  ArrowLeft,
  CheckCircle2,
  AlertTriangle,
  FileText,
  Image as ImageIcon,
  Video,
  Music,
  Archive,
  Eye,
  Wrench,
  Download,
  Moon,
  Sun,
  X,
  Play,
  Square,
  LayoutGrid,
  List,
  Check,
  Sliders,
  FolderCheck,
  Database,
  Terminal,
  Copy,
  Trash2,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  Volume2,
  FileCode,
} from 'lucide-react';

interface DeviceInfo {
  id: string;
  name: string;
  device_type: string;
  mount_point?: string;
  size_bytes: number;
  filesystem?: string;
  is_read_only: boolean;
  requires_elevation: boolean;
}

interface RecoveredFile {
  id: string;
  filename: string;
  original_path?: string;
  category: 'Image' | 'Document' | 'Video' | 'Audio' | 'Archive' | 'Text' | 'Other';
  extension: string;
  size_bytes: number;
  source_offset: number;
  integrity: string;
  created_at?: string;
  modified_at?: string;
  metadata_source: string;
  sha256?: string;
}

interface HexLine {
  offset: string;
  hex: string;
  ascii: string;
}

interface PreviewData {
  preview_type: string;
  media_data_url?: string;
  thumbnail_base64?: string;
  dimensions?: [number, number];
  hex_dump: HexLine[];
  text_snippet?: string;
  metadata_entries: [string, string][];
}

interface RepairReport {
  original_size: number;
  repaired_size: number;
  detected_format: string;
  issues_found: string[];
  fixes_applied: string[];
  success: boolean;
  saved?: boolean;
  saved_path?: string;
  user_cancelled?: boolean;
}

interface LogEntry {
  id: string;
  timestamp: string;
  level: 'info' | 'warn' | 'error' | 'success';
  message: string;
}

export default function App() {
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');
  const [language, setLanguage] = useState<'en' | 'pt'>('en');
  const [currentStep, setCurrentStep] = useState<number>(0); // 0 = Dashboard, 1..7 = Workflow
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [selectedSource, setSelectedSource] = useState<DeviceInfo | null>(null);
  const [scanMode, setScanMode] = useState<'quick' | 'deep' | 'image'>('quick');
  const [selectedCategories, setSelectedCategories] = useState<string[]>([
    'Image', 'Document', 'Video', 'Audio', 'Archive', 'Text', 'Other'
  ]);

  // Engine Settings
  const [scanStride, setScanStride] = useState<number>(512);
  const [enforceSeparation, setEnforceSeparation] = useState<boolean>(true);
  const [autoRepairMarkers, setAutoRepairMarkers] = useState<boolean>(true);
  const [maxBufferSizeMb, setMaxBufferSizeMb] = useState<number>(128);

  // Scan state
  const [isScanning, setIsScanning] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);
  const [scanProgress, setScanProgress] = useState({
    percent: 0,
    scannedBytes: 0,
    totalBytes: 0,
    speedMBps: 0,
    filesCount: 0,
    currentLabel: 'Ready',
  });
  const [discoveredFiles, setDiscoveredFiles] = useState<RecoveredFile[]>([]);
  const [selectedFileIds, setSelectedFileIds] = useState<Set<string>>(new Set());

  // Pagination state
  const [pageSize, setPageSize] = useState<number>(50);
  const [currentPage, setCurrentPage] = useState<number>(1);

  // Preview Drawer tab ('media' | 'hex')
  const [previewTab, setPreviewTab] = useState<'media' | 'hex'>('media');
  const [copiedHex, setCopiedHex] = useState<boolean>(false);

  // Terminal & Diagnostics state
  const [showTerminal, setShowTerminal] = useState<boolean>(true);
  const [terminalHeight, setTerminalHeight] = useState<number>(160);
  const [isDraggingTerminal, setIsDraggingTerminal] = useState<boolean>(false);
  const [terminalLogs, setTerminalLogs] = useState<LogEntry[]>([
    {
      id: 'init-1',
      timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
      level: 'info',
      message: 'OpenRe v0.1.5 engine initialized in strict offline-first mode. Zero telemetry active.',
    },
    {
      id: 'init-2',
      timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
      level: 'success',
      message: 'All device & partition readers configured for strictly non-destructive read-only operation.',
    },
  ]);
  const terminalEndRef = React.useRef<HTMLDivElement | null>(null);

  // Preloader & Indexing state
  const [isPreparingResults, setIsPreparingResults] = useState<boolean>(false);
  const [preparingCount, setPreparingCount] = useState<number>(0);

  // Image thumbnails cache: { [fileId]: dataUrl }
  const [imageThumbnails, setImageThumbnails] = useState<Record<string, string>>({});
  const requestedThumbnailsRef = useRef<Set<string>>(new Set());
  const activePreviewIdRef = useRef<string | null>(null);

  const appendLog = (level: 'info' | 'warn' | 'error' | 'success', message: string, timestamp?: string) => {
    const timeStr = timestamp || new Date().toLocaleTimeString('en-US', { hour12: false });
    const newEntry: LogEntry = {
      id: Math.random().toString(36).substring(2, 9),
      timestamp: timeStr,
      level,
      message,
    };
    setTerminalLogs((prev) => [...prev.slice(-300), newEntry]);
  };

  useEffect(() => {
    if (showTerminal && terminalEndRef.current) {
      terminalEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [terminalLogs, showTerminal]);

  // Terminal vertical resize listener
  useEffect(() => {
    if (!isDraggingTerminal) return;
    const handleMouseMove = (e: MouseEvent) => {
      const newHeight = window.innerHeight - e.clientY;
      setTerminalHeight(Math.max(80, Math.min(550, newHeight)));
    };
    const handleMouseUp = () => {
      setIsDraggingTerminal(false);
    };
    document.body.style.cursor = 'row-resize';
    document.body.style.userSelect = 'none';
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDraggingTerminal]);


  // Preview Drawer state
  const [previewFile, setPreviewFile] = useState<RecoveredFile | null>(null);
  const [previewData, setPreviewData] = useState<PreviewData | null>(null);
  const [repairReport, setRepairReport] = useState<RepairReport | null>(null);

  // Recovery state
  const [destPath, setDestPath] = useState<string>('');
  const [isRecovering, setIsRecovering] = useState(false);
  const [recoveryDone, setRecoveryDone] = useState(false);
  const [recoveryReportMd, setRecoveryReportMd] = useState<string>('');
  const [collisionWarning, setCollisionWarning] = useState<string | null>(null);

  // View state
  const [viewMode, setViewMode] = useState<'list' | 'grid'>('list');
  const [searchQuery, setSearchQuery] = useState('');
  const [filterCategory, setFilterCategory] = useState<string>('All');

  // Modals
  const [showSettings, setShowSettings] = useState(false);
  const [showAbout, setShowAbout] = useState(false);

  // IPC Bridge Helper
  const sendIpc = (cmd: string, payload: any = {}): Promise<any> => {
    return new Promise((resolve) => {
      const msgId = Math.random().toString(36).substring(2);
      const handler = (event: any) => {
        if (event.detail && event.detail.msgId === msgId) {
          window.removeEventListener('openre-response' as any, handler);
          let data = event.detail.data;
          if (typeof data === 'string') {
            try { data = JSON.parse(data); } catch (_) {}
          }
          resolve(data);
        }
      };
      window.addEventListener('openre-response' as any, handler);

      if ((window as any).ipc) {
        (window as any).ipc.postMessage(JSON.stringify({ cmd, msgId, payload }));
      } else {
        simulateIpc(cmd, payload, resolve);
      }
    });
  };

  const simulateIpc = (cmd: string, payload: any, resolve: (res: any) => void) => {
    if (cmd === 'list_devices') {
      resolve([
        { id: 'C:', name: 'Local Disk (C:)', device_type: 'Volume', size_bytes: 512110190592, is_read_only: false, requires_elevation: false },
        { id: 'D:', name: 'Data Drive (D:)', device_type: 'Volume', size_bytes: 1000204886016, is_read_only: false, requires_elevation: false },
        { id: 'E:', name: 'USB Flash Drive (E:)', device_type: 'Volume', size_bytes: 32000000000, is_read_only: false, requires_elevation: false },
        { id: 'I:', name: 'Backup Drive (I:)', device_type: 'Volume', size_bytes: 2000000000000, is_read_only: false, requires_elevation: false },
        { id: '\\\\.\\PhysicalDrive0', name: 'Physical Drive 0 (NVMe SSD)', device_type: 'PhysicalDrive', size_bytes: 512110190592, is_read_only: false, requires_elevation: true },
        { id: '\\\\.\\PhysicalDrive1', name: 'Physical Drive 1 (SATA HDD)', device_type: 'PhysicalDrive', size_bytes: 1000204886016, is_read_only: false, requires_elevation: true },
      ]);
    } else if (cmd === 'select_folder') {
      resolve('D:\\OpenRe_Recovered_Files');
    } else if (cmd === 'select_image_file') {
      resolve('D:\\backups\\disk_dump.img');
    } else {
      resolve({});
    }
  };

  useEffect(() => {
    sendIpc('list_devices').then((devs) => {
      if (Array.isArray(devs)) setDevices(devs);
    });

    const onBackendEvent = (e: any) => {
      let { type, payload } = e.detail || {};
      if (typeof payload === 'string') {
        try { payload = JSON.parse(payload); } catch (_) {}
      }
      if (!payload) payload = {};

      if (type === 'openre-log') {
        appendLog(payload.level || 'info', payload.message || '', payload.timestamp);
      } else if (type === 'scan_progress') {
        const pct = payload.total_bytes > 0 ? (payload.bytes_scanned / payload.total_bytes) * 100 : 0;
        setScanProgress({
          percent: Math.min(100, Math.round(pct)),
          scannedBytes: payload.bytes_scanned,
          totalBytes: payload.total_bytes,
          speedMBps: Math.round((payload.speed_bytes_per_sec / (1024 * 1024)) * 10) / 10,
          filesCount: payload.files_found,
          currentLabel: payload.current_label || 'Scanning...',
        });
      } else if (type === 'scan_completed') {
        setIsScanning(false);
        setScanError(null);
        const filesList = Array.isArray(payload.files) ? payload.files : [];
        setPreparingCount(filesList.length);
        setIsPreparingResults(true);
        setCurrentStep(5);

        appendLog(
          'info',
          language === 'en'
            ? `Scan finalized. Structuring and indexing catalog for ${filesList.length} files...`
            : `Varredura finalizada. Estruturando e indexando catálogo para ${filesList.length} arquivos...`
        );

        setTimeout(() => {
          setDiscoveredFiles(filesList);
          setSelectedFileIds(new Set(filesList.map((f: any) => f.id)));
          if (filesList.length > 0) {
            openPreview(filesList[0]);
          }
          setTimeout(() => {
            setIsPreparingResults(false);
            appendLog(
              'success',
              language === 'en'
                ? `Catalog ready. Displaying ${filesList.length} recoverable items.`
                : `Catálogo pronto. Exibindo ${filesList.length} itens recuperáveis.`
            );
          }, 850);
        }, 200);
      } else if (type === 'scan_error') {
        setIsScanning(false);
        setScanError(payload.error || 'Scan stopped due to an error.');
        setScanProgress((prev) => ({
          ...prev,
          currentLabel: `Scan halted: ${payload.error}`,
        }));
        appendLog('error', `Scan halted: ${payload.error}`);
      }
    };

    window.addEventListener('openre-event' as any, onBackendEvent);
    return () => window.removeEventListener('openre-event' as any, onBackendEvent);
  }, []);

  const handleStartScan = () => {
    if (!selectedSource) return;
    setImageThumbnails({});
    requestedThumbnailsRef.current.clear();
    setIsScanning(true);
    setScanError(null);
    setCurrentStep(4);
    appendLog(
      'info',
      language === 'en'
        ? `Starting ${scanMode.toUpperCase()} scan on ${selectedSource.name}...`
        : `Iniciando varredura ${scanMode === 'quick' ? 'RÁPIDA' : 'PROFUNDA'} em ${selectedSource.name}...`
    );
    setScanProgress({
      percent: 0,
      scannedBytes: 0,
      totalBytes: selectedSource.size_bytes || 1024 * 1024 * 1024,
      speedMBps: 0,
      filesCount: 0,
      currentLabel: language === 'en' ? 'Starting scan...' : 'Iniciando varredura...',
    });

    sendIpc('start_scan', {
      source_id: selectedSource.id,
      mode: scanMode,
      categories: selectedCategories,
    });
  };

  const handleCancelScan = () => {
    appendLog(
      'warn',
      language === 'en'
        ? 'Scan stop requested by user. Finalizing and preserving all discovered files...'
        : 'Parada solicitada pelo usuário. Finalizando e preservando todos os arquivos localizados...'
    );
    sendIpc('cancel_scan');
  };


  const handleSelectDestFolder = async () => {
    const selected = await sendIpc('select_folder');
    if (selected) {
      setDestPath(selected);
      if (selectedSource && enforceSeparation) {
        const sourceLetter = selectedSource.id.substring(0, 2).toUpperCase();
        const destLetter = selected.substring(0, 2).toUpperCase();
        if (sourceLetter.endsWith(':') && sourceLetter === destLetter) {
          setCollisionWarning(
            language === 'en'
              ? `Destination drive (${destLetter}) matches the source drive being recovered! Writing files to the same drive overwrites deleted data. Select a different drive.`
              : `A unidade de destino (${destLetter}) é a mesma unidade sendo recuperada! Gravar arquivos na mesma unidade sobrescreve dados apagados. Selecione uma unidade diferente.`
          );
        } else {
          setCollisionWarning(null);
        }
      }
    }
  };

  const handleStartRecovery = async () => {
    if (!destPath || (collisionWarning && enforceSeparation)) return;
    setIsRecovering(true);
    const targetFiles = discoveredFiles.filter((f) => selectedFileIds.has(f.id));

    const res = await sendIpc('recover_files', {
      source_id: selectedSource?.id,
      files: targetFiles,
      destination_path: destPath,
    });

    setIsRecovering(false);
    setRecoveryDone(true);
    setRecoveryReportMd(
      res?.report_markdown ||
        (language === 'en' ? 'Recovery report generated successfully.' : 'Relatório de recuperação gerado com sucesso.')
    );
    setCurrentStep(7);
  };

  const openPreview = async (file: RecoveredFile) => {
    activePreviewIdRef.current = file.id;
    setPreviewFile(file);
    setRepairReport(null);
    setPreviewTab('media');

    // If thumbnail already cached for image, pre-seed previewData for instant visual feedback
    if (file.category === 'Image' && imageThumbnails[file.id]) {
      setPreviewData({
        preview_type: 'image',
        thumbnail_base64: imageThumbnails[file.id],
        media_data_url: imageThumbnails[file.id],
        hex_dump: [],
        metadata_entries: [
          [language === 'en' ? 'Format' : 'Formato', file.extension.toUpperCase()],
          [language === 'en' ? 'File Size' : 'Tamanho do Arquivo', `${(file.size_bytes / 1024).toFixed(1)} KB`],
        ],
      });
    } else {
      setPreviewData(null);
    }

    const data = await sendIpc('get_file_preview', {
      source_id: selectedSource?.id,
      source_offset: file.source_offset,
      size_bytes: file.size_bytes,
      extension: file.extension,
      original_path: file.original_path,
    });
    if (activePreviewIdRef.current === file.id) {
      setPreviewData(data);
    }
  };

  const handleRepairCopy = async () => {
    if (!previewFile) return;
    const report = await sendIpc('repair_file', {
      source_id: selectedSource?.id,
      source_offset: previewFile.source_offset,
      size_bytes: previewFile.size_bytes,
      filename: previewFile.filename,
      original_path: previewFile.original_path,
    });
    setRepairReport(report);
  };

  const handleCopyHexDump = () => {
    if (!previewData?.hex_dump || previewData.hex_dump.length === 0) return;
    const text = previewData.hex_dump
      .map((line) => `${line.offset}  ${line.hex}  |${line.ascii}|`)
      .join('\n');
    navigator.clipboard.writeText(text);
    setCopiedHex(true);
    setTimeout(() => setCopiedHex(false), 2000);
  };

  const filteredFiles = discoveredFiles.filter((f) => {
    const matchCat = filterCategory === 'All' || f.category === filterCategory;
    const matchSearch =
      searchQuery === '' ||
      f.filename.toLowerCase().includes(searchQuery.toLowerCase()) ||
      f.extension.toLowerCase().includes(searchQuery.toLowerCase());
    return matchCat && matchSearch;
  });

  // Reset to page 1 when query, category, or page size changes
  useEffect(() => {
    setCurrentPage(1);
  }, [searchQuery, filterCategory, pageSize]);

  // Pagination computations
  const totalItems = filteredFiles.length;
  const totalPages = Math.max(1, Math.ceil(totalItems / pageSize));
  const safeCurrentPage = Math.min(Math.max(1, currentPage), totalPages);
  const startIndex = (safeCurrentPage - 1) * pageSize;
  const endIndex = Math.min(startIndex + pageSize, totalItems);
  const paginatedFiles = filteredFiles.slice(startIndex, endIndex);

  const isPageFullySelected =
    paginatedFiles.length > 0 && paginatedFiles.every((f) => selectedFileIds.has(f.id));
  const isAllFullySelected =
    filteredFiles.length > 0 && filteredFiles.every((f) => selectedFileIds.has(f.id));

  const toggleSelectPage = () => {
    const next = new Set(selectedFileIds);
    if (isPageFullySelected) {
      paginatedFiles.forEach((f) => next.delete(f.id));
    } else {
      paginatedFiles.forEach((f) => next.add(f.id));
    }
    setSelectedFileIds(next);
  };

  const toggleSelectAllFiltered = () => {
    if (isAllFullySelected) {
      setSelectedFileIds(new Set());
    } else {
      setSelectedFileIds(new Set(filteredFiles.map((f) => f.id)));
    }
  };

  // Batch fetch thumbnails for visible images on current page
  useEffect(() => {
    if (currentStep !== 5) return;
    const imageFiles = paginatedFiles.filter(
      (f) => f.category === 'Image' && !imageThumbnails[f.id] && !requestedThumbnailsRef.current.has(f.id)
    );
    if (imageFiles.length === 0) return;

    imageFiles.forEach((f) => requestedThumbnailsRef.current.add(f.id));

    const batchPayload = imageFiles.map((f) => ({
      id: f.id,
      source_id: selectedSource?.id,
      source_offset: f.source_offset,
      size_bytes: f.size_bytes,
      extension: f.extension,
      original_path: f.original_path,
    }));

    sendIpc('get_thumbnails', { items: batchPayload })
      .then((res: any) => {
        if (res && res.thumbnails && Object.keys(res.thumbnails).length > 0) {
          setImageThumbnails((prev) => ({ ...prev, ...res.thumbnails }));
        }
      })
      .catch((err) => {
        console.error('Failed to load thumbnails:', err);
      });
  }, [paginatedFiles, currentStep, selectedSource, imageThumbnails]);

  const getCategoryIcon = (cat: string) => {
    switch (cat) {
      case 'Image': return <ImageIcon className="w-4 h-4 text-emerald-400" />;
      case 'Document': return <FileText className="w-4 h-4 text-blue-400" />;
      case 'Video': return <Video className="w-4 h-4 text-purple-400" />;
      case 'Audio': return <Music className="w-4 h-4 text-amber-400" />;
      case 'Archive': return <Archive className="w-4 h-4 text-orange-400" />;
      default: return <FileSearch className="w-4 h-4 text-slate-400" />;
    }
  };

  const getCategoryName = (cat: string) => {
    if (language === 'en') return cat;
    switch (cat) {
      case 'Image': return 'Imagem';
      case 'Document': return 'Documento';
      case 'Video': return 'Vídeo';
      case 'Audio': return 'Áudio';
      case 'Archive': return 'Compactado';
      case 'Text': return 'Texto';
      default: return cat;
    }
  };

  const getIntegrityBadge = (status: string) => {
    if (status.includes('Likely')) {
      return (
        <span className="px-2 py-0.5 text-xs rounded-full bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
          {language === 'en' ? 'Likely Intact' : 'Provável Íntegro'}
        </span>
      );
    }
    if (status.includes('Partially')) {
      return (
        <span className="px-2 py-0.5 text-xs rounded-full bg-amber-500/20 text-amber-400 border border-amber-500/30">
          {language === 'en' ? 'Partial' : 'Parcial'}
        </span>
      );
    }
    return (
      <span className="px-2 py-0.5 text-xs rounded-full bg-rose-500/20 text-rose-400 border border-rose-500/30">
        {language === 'en' ? 'Corrupted' : 'Corrompido'}
      </span>
    );
  };

  return (
    <div className={`h-screen w-screen overflow-hidden flex flex-col ${theme === 'dark' ? 'bg-slate-950 text-slate-100' : 'bg-slate-50 text-slate-900'}`}>
      {/* Top Header */}
      <header className="h-14 shrink-0 border-b border-slate-800 px-6 flex items-center justify-between bg-slate-900/60 backdrop-blur select-none">
        <div className="flex items-center gap-3 cursor-pointer" onClick={() => setCurrentStep(0)}>
          <div className="w-8 h-8 rounded-lg bg-gradient-to-tr from-sky-600 to-indigo-500 flex items-center justify-center font-bold text-white shadow-lg shadow-sky-500/20">
            Re
          </div>
          <div>
            <div className="font-semibold text-base tracking-wide flex items-center gap-2">
              OpenRe
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-sky-500/20 text-sky-400 border border-sky-500/30">v0.1.5</span>
            </div>
            <div className="text-[11px] text-slate-400">
              {language === 'en' ? 'Professional File Recovery' : 'Recuperação Profissional de Arquivos'}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 text-xs">
            <ShieldCheck className="w-3.5 h-3.5" />
            <span>{language === 'en' ? '100% Offline & Private' : '100% Offline & Privativo'}</span>
          </div>

          {/* Diagnostic Terminal Toggle Button */}
          <button
            onClick={() => setShowTerminal(!showTerminal)}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg border text-xs font-mono transition ${
              showTerminal
                ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-400'
                : 'bg-slate-800/80 border-slate-700 text-slate-400 hover:text-slate-200'
            }`}
            title={showTerminal ? (language === 'en' ? 'Hide Terminal' : 'Ocultar Terminal (Rodapé)') : (language === 'en' ? 'Show Terminal' : 'Exibir Terminal (Rodapé)')}
          >
            <Terminal className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Terminal</span>
            {terminalLogs.length > 0 && (
              <span className="text-[10px] px-1 rounded bg-black/40 text-slate-300">
                {terminalLogs.length}
              </span>
            )}
          </button>

          <button
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-100 hover:bg-slate-800 transition"
            title={language === 'en' ? 'Toggle theme' : 'Alternar tema claro/escuro'}
          >
            {theme === 'dark' ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
          </button>

          <button
            onClick={() => setShowSettings(true)}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-100 hover:bg-slate-800 transition relative"
            title={language === 'en' ? 'Open Settings' : 'Abrir Configurações'}
          >
            <Settings className="w-4 h-4" />
          </button>

          <button
            onClick={() => setShowAbout(true)}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-100 hover:bg-slate-800 transition"
            title={language === 'en' ? 'About OpenRe' : 'Sobre o OpenRe'}
          >
            <Info className="w-4 h-4" />
          </button>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1 min-h-0 overflow-hidden flex flex-col p-6">
        {/* Step Indicator Bar when in workflow */}
        {currentStep > 0 && (
          <div className="shrink-0 mb-4 flex items-center justify-between border-b border-slate-800 pb-3">
            <div className="flex items-center gap-2">
              {(language === 'en'
                ? ['Source', 'Method', 'Filters', 'Scan', 'Review', 'Recover', 'Report']
                : ['Origem', 'Método', 'Filtros', 'Varredura', 'Revisão', 'Recuperar', 'Relatório']
              ).map((label, idx) => {
                const stepNum = idx + 1;
                const isCurrent = currentStep === stepNum;
                const isPassed = currentStep > stepNum;
                return (
                  <div key={label} className="flex items-center">
                    <div
                      className={`flex items-center gap-2 px-3 py-1 rounded-lg text-xs font-medium transition ${
                        isCurrent
                          ? 'bg-sky-500 text-white shadow-md shadow-sky-500/25'
                          : isPassed
                          ? 'bg-slate-800 text-slate-300'
                          : 'text-slate-500'
                      }`}
                    >
                      <span>{stepNum}</span>
                      <span>{label}</span>
                    </div>
                    {idx < 6 && <div className="w-3 h-[1px] bg-slate-800 mx-1" />}
                  </div>
                );
              })}
            </div>

            {currentStep < 4 && (
              <button
                onClick={() => setCurrentStep(0)}
                className="text-xs text-slate-400 hover:text-slate-200 transition"
              >
                {language === 'en' ? 'Cancel to Dashboard' : 'Cancelar e Voltar'}
              </button>
            )}
          </div>
        )}

        {/* STEP 0: Initial Screen / Dashboard */}
        {currentStep === 0 && (
          <div className="flex-1 min-h-0 overflow-y-auto flex flex-col items-center justify-center max-w-4xl mx-auto w-full">
            <div className="text-center mb-8">
              <h1 className="text-3xl font-bold tracking-tight mb-2">
                {language === 'en' ? 'Recover Your Lost Files Safely' : 'Recupere seus Arquivos com Segurança'}
              </h1>
              <p className="text-slate-400 max-w-xl mx-auto text-sm">
                {language === 'en'
                  ? 'OpenRe performs high-performance deep sector carving and filesystem undeletion with guaranteed offline privacy.'
                  : 'O OpenRe realiza escavação profunda de setores e restauração de arquivos apagados com privacidade 100% offline garantida.'}
              </p>
            </div>

            <div className="grid grid-cols-3 gap-4 w-full mb-8">
              <div
                onClick={() => { setScanMode('quick'); setCurrentStep(1); }}
                className="p-5 rounded-xl border border-slate-800 bg-slate-900/50 hover:border-sky-500/50 hover:bg-slate-800/50 cursor-pointer transition flex flex-col group shadow-sm"
              >
                <div className="w-10 h-10 rounded-lg bg-sky-500/10 text-sky-400 flex items-center justify-center mb-4 group-hover:scale-110 transition">
                  <FileSearch className="w-5 h-5" />
                </div>
                <h3 className="font-semibold text-base mb-1">
                  {language === 'en' ? 'Recover Files' : 'Recuperar Arquivos'}
                </h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  {language === 'en'
                    ? 'Step-by-step guided recovery wizard for deleted files, pictures, and documents.'
                    : 'Assistente guiado passo a passo para arquivos excluídos, fotos e documentos.'}
                </p>
                <div className="text-xs font-medium text-sky-400 flex items-center gap-1">
                  {language === 'en' ? 'Start Wizard' : 'Iniciar Assistente'} <ArrowRight className="w-3.5 h-3.5" />
                </div>
              </div>

              <div
                onClick={() => { setScanMode('deep'); setCurrentStep(1); }}
                className="p-5 rounded-xl border border-slate-800 bg-slate-900/50 hover:border-indigo-500/50 hover:bg-slate-800/50 cursor-pointer transition flex flex-col group shadow-sm"
              >
                <div className="w-10 h-10 rounded-lg bg-indigo-500/10 text-indigo-400 flex items-center justify-center mb-4 group-hover:scale-110 transition">
                  <HardDrive className="w-5 h-5" />
                </div>
                <h3 className="font-semibold text-base mb-1">
                  {language === 'en' ? 'Scan a Drive' : 'Varrer uma Unidade'}
                </h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  {language === 'en'
                    ? 'Deep block-strided signature carving across damaged, unpartitioned, or formatted disks.'
                    : 'Escavação profunda por assinatura em setores de discos danificados ou formatados.'}
                </p>
                <div className="text-xs font-medium text-indigo-400 flex items-center gap-1">
                  {language === 'en' ? 'Select Drive' : 'Selecionar Unidade'} <ArrowRight className="w-3.5 h-3.5" />
                </div>
              </div>

              <div
                onClick={async () => {
                  const img = await sendIpc('select_image_file');
                  if (img) {
                    setSelectedSource({
                      id: img,
                      name: `Disk Image (${img.split(/[\\/]/).pop()})`,
                      device_type: 'DiskImage',
                      size_bytes: 0,
                      is_read_only: true,
                      requires_elevation: false,
                    });
                    setScanMode('deep');
                    setCurrentStep(2);
                  }
                }}
                className="p-5 rounded-xl border border-slate-800 bg-slate-900/50 hover:border-emerald-500/50 hover:bg-slate-800/50 cursor-pointer transition flex flex-col group shadow-sm"
              >
                <div className="w-10 h-10 rounded-lg bg-emerald-500/10 text-emerald-400 flex items-center justify-center mb-4 group-hover:scale-110 transition">
                  <FolderOpen className="w-5 h-5" />
                </div>
                <h3 className="font-semibold text-base mb-1">
                  {language === 'en' ? 'Scan a Disk Image' : 'Varrer Imagem de Disco'}
                </h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  {language === 'en'
                    ? 'Load raw image dumps (.img, .dd, .raw) safely without accessing the physical media repeatedly.'
                    : 'Carregue imagens brutas (.img, .dd, .raw) sem acessar a mídia física repetidamente.'}
                </p>
                <div className="text-xs font-medium text-emerald-400 flex items-center gap-1">
                  {language === 'en' ? 'Browse Image' : 'Procurar Imagem'} <ArrowRight className="w-3.5 h-3.5" />
                </div>
              </div>
            </div>

            <div className="text-xs text-slate-500 flex items-center gap-6">
              <span>✓ NTFS / FAT32 / exFAT / ext4</span>
              <span>✓ {language === 'en' ? '15+ Format-Aware Carvers' : 'Mais de 15 Reconhecedores de Formatos'}</span>
              <span>✓ {language === 'en' ? 'Pre-Recovery Media Sandbox Preview' : 'Pré-visualização Segura de Mídia e Hex'}</span>
            </div>
          </div>
        )}

        {/* STEP 1: Select Source (Fixed scrolling & visible footer) */}
        {currentStep === 1 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-4xl mx-auto w-full">
            <div className="shrink-0 mb-3 flex items-center justify-between">
              <div>
                <h2 className="text-xl font-bold">
                  {language === 'en' ? 'Step 1 — Select Storage Media' : 'Etapa 1 — Selecionar Mídia de Armazenamento'}
                </h2>
                <p className="text-xs text-slate-400">
                  {language === 'en'
                    ? 'Select a drive below, or double-click to proceed directly.'
                    : 'Selecione uma unidade abaixo ou clique duas vezes para avançar diretamente.'}
                </p>
              </div>

              {/* Quick browse shortcuts */}
              <div className="flex items-center gap-2">
                <button
                  onClick={async () => {
                    const fld = await sendIpc('select_folder');
                    if (fld) {
                      setSelectedSource({
                        id: fld,
                        name: `Folder (${fld.split(/[\\/]/).pop()})`,
                        device_type: 'Directory',
                        size_bytes: 0,
                        is_read_only: false,
                        requires_elevation: false,
                      });
                      setCurrentStep(2);
                    }
                  }}
                  className="px-3 py-1.5 rounded-lg border border-slate-700 bg-slate-900 hover:bg-slate-800 text-xs text-slate-200 transition flex items-center gap-1.5"
                >
                  <FolderOpen className="w-3.5 h-3.5 text-sky-400" />
                  <span>{language === 'en' ? 'Browse Folder...' : 'Procurar Pasta...'}</span>
                </button>

                <button
                  onClick={async () => {
                    const img = await sendIpc('select_image_file');
                    if (img) {
                      setSelectedSource({
                        id: img,
                        name: `Disk Image (${img.split(/[\\/]/).pop()})`,
                        device_type: 'DiskImage',
                        size_bytes: 0,
                        is_read_only: true,
                        requires_elevation: false,
                      });
                      setCurrentStep(2);
                    }
                  }}
                  className="px-3 py-1.5 rounded-lg border border-slate-700 bg-slate-900 hover:bg-slate-800 text-xs text-slate-200 transition flex items-center gap-1.5"
                >
                  <Database className="w-3.5 h-3.5 text-emerald-400" />
                  <span>{language === 'en' ? 'Browse Image (.img)...' : 'Procurar Imagem (.img)...'}</span>
                </button>
              </div>
            </div>

            {/* Scrollable Device List Container with Min-H-0 */}
            <div className="flex-1 min-h-0 overflow-y-auto pr-3 space-y-2 border border-slate-800/80 rounded-xl p-3 bg-slate-950/40">
              <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-400 mb-1 flex items-center justify-between">
                <span>{language === 'en' ? 'Logical Partitions & Volumes' : 'Partições Lógicas & Volumes'}</span>
                <span className="text-slate-500 text-[10px]">
                  {language === 'en' ? 'Click to select, double-click to advance' : 'Clique para selecionar, duplo-clique para avançar'}
                </span>
              </div>
              {devices.filter((d) => d.device_type === 'Volume').map((dev) => {
                const isSelected = selectedSource?.id === dev.id;
                return (
                  <div
                    key={dev.id}
                    onClick={() => setSelectedSource(dev)}
                    onDoubleClick={() => { setSelectedSource(dev); setCurrentStep(2); }}
                    className={`p-3 rounded-lg border flex items-center justify-between cursor-pointer transition select-none ${
                      isSelected
                        ? 'border-sky-500 bg-sky-500/15 ring-2 ring-sky-500/30'
                        : 'border-slate-800/80 bg-slate-900/40 hover:border-slate-700 hover:bg-slate-900/70'
                    }`}
                  >
                    <div className="flex items-center gap-3">
                      <HardDrive className={`w-5 h-5 ${isSelected ? 'text-sky-400' : 'text-slate-400'}`} />
                      <div>
                        <div className="font-semibold text-sm text-slate-200 flex items-center gap-2">
                          {dev.name}
                          {isSelected && <Check className="w-4 h-4 text-sky-400" />}
                        </div>
                        <div className="text-xs text-slate-400">{dev.mount_point || dev.id}</div>
                      </div>
                    </div>
                    <div className="text-xs font-mono text-slate-300">
                      {dev.size_bytes > 0 ? `${(dev.size_bytes / (1024 * 1024 * 1024)).toFixed(1)} GB` : (language === 'en' ? 'Available' : 'Disponível')}
                    </div>
                  </div>
                );
              })}

              <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-400 mt-5 mb-1">
                {language === 'en' ? 'Physical Hard Drives & SSDs' : 'Discos Físicos & SSDs'}
              </div>
              {devices.filter((d) => d.device_type === 'PhysicalDrive').map((dev) => {
                const isSelected = selectedSource?.id === dev.id;
                return (
                  <div
                    key={dev.id}
                    onClick={() => setSelectedSource(dev)}
                    onDoubleClick={() => { setSelectedSource(dev); setCurrentStep(2); }}
                    className={`p-3 rounded-lg border flex items-center justify-between cursor-pointer transition select-none ${
                      isSelected
                        ? 'border-indigo-500 bg-indigo-500/15 ring-2 ring-indigo-500/30'
                        : 'border-slate-800/80 bg-slate-900/40 hover:border-slate-700 hover:bg-slate-900/70'
                    }`}
                  >
                    <div className="flex items-center gap-3">
                      <HardDrive className="w-5 h-5 text-indigo-400" />
                      <div>
                        <div className="font-semibold text-sm text-slate-200 flex items-center gap-2">
                          {dev.name}
                          {isSelected && <Check className="w-4 h-4 text-indigo-400" />}
                        </div>
                        <div className="text-xs text-slate-400">{dev.id}</div>
                      </div>
                    </div>
                    <span className="text-[10px] px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
                      {language === 'en' ? 'Admin Elevation' : 'Requer Elevação'}
                    </span>
                  </div>
                );
              })}
            </div>

            {/* Pinned Sticky Footer - ALWAYS VISIBLE */}
            <div className="shrink-0 pt-3 border-t border-slate-800 flex items-center justify-between bg-slate-950/60 backdrop-blur">
              <button
                onClick={() => setCurrentStep(0)}
                className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800 transition"
              >
                {language === 'en' ? 'Back to Dashboard' : 'Voltar ao Início'}
              </button>

              <div className="flex items-center gap-4">
                <span className="text-xs text-slate-400">
                  {selectedSource ? (
                    <span className="text-slate-200 font-medium">
                      {language === 'en' ? 'Selected: ' : 'Selecionado: '}
                      <strong>{selectedSource.name}</strong>
                    </span>
                  ) : (
                    language === 'en' ? 'Select a drive to proceed' : 'Selecione uma unidade para continuar'
                  )}
                </span>
                <button
                  disabled={!selectedSource}
                  onClick={() => setCurrentStep(2)}
                  className="px-6 py-2.5 rounded-lg bg-sky-600 hover:bg-sky-500 disabled:opacity-40 text-xs font-semibold text-white shadow-md shadow-sky-600/25 transition flex items-center gap-2"
                >
                  {language === 'en' ? 'Continue to Recovery Method' : 'Continuar para Método de Recuperação'}{' '}
                  <ArrowRight className="w-4 h-4" />
                </button>
              </div>
            </div>
          </div>
        )}

        {/* STEP 2: Select Method */}
        {currentStep === 2 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-4xl mx-auto w-full">
            <h2 className="text-xl font-bold mb-1">
              {language === 'en' ? 'Step 2 — Select Recovery Method' : 'Etapa 2 — Selecionar Método de Recuperação'}
            </h2>
            <p className="text-xs text-slate-400 mb-6">
              {language === 'en'
                ? 'Choose how OpenRe should analyze the selected media.'
                : 'Escolha como o OpenRe deve analisar a mídia selecionada.'}
            </p>

            <div className="grid grid-cols-2 gap-4 mb-6">
              <div
                onClick={() => setScanMode('quick')}
                className={`p-5 rounded-xl border cursor-pointer transition flex flex-col ${
                  scanMode === 'quick'
                    ? 'border-sky-500 bg-sky-500/10 ring-2 ring-sky-500/30'
                    : 'border-slate-800 bg-slate-900/40 hover:border-slate-700'
                }`}
              >
                <div className="w-8 h-8 rounded-lg bg-sky-500/20 text-sky-400 flex items-center justify-center mb-3">
                  <FileSearch className="w-4 h-4" />
                </div>
                <h3 className="font-semibold text-sm mb-1">
                  {language === 'en' ? 'Quick Scan (Filesystem Undelete)' : 'Varredura Rápida (Restauração do Sistema de Arquivos)'}
                </h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  {language === 'en'
                    ? 'Fastest option. Inspects NTFS ($MFT), FAT32, exFAT, and ext4 directory metadata to recover recently deleted files with original filenames, timestamps, and directories intact.'
                    : 'Opção mais rápida. Inspeciona metadados NTFS ($MFT), FAT32, exFAT e ext4 para restaurar arquivos excluídos recentemente mantendo nomes originais, datas e pastas intactos.'}
                </p>
                <div className="text-xs text-emerald-400 font-medium">
                  {language === 'en' ? '✓ Preserves original file names & paths' : '✓ Preserva nomes e caminhos originais de arquivos'}
                </div>
              </div>

              <div
                onClick={() => setScanMode('deep')}
                className={`p-5 rounded-xl border cursor-pointer transition flex flex-col ${
                  scanMode === 'deep'
                    ? 'border-indigo-500 bg-indigo-500/10 ring-2 ring-indigo-500/30'
                    : 'border-slate-800 bg-slate-900/40 hover:border-slate-700'
                }`}
              >
                <div className="w-8 h-8 rounded-lg bg-indigo-500/20 text-indigo-400 flex items-center justify-center mb-3">
                  <HardDrive className="w-4 h-4" />
                </div>
                <h3 className="font-semibold text-sm mb-1">
                  {language === 'en' ? 'Deep Scan (Signature Carving)' : 'Varredura Profunda (Escavação por Assinatura)'}
                </h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  {language === 'en'
                    ? 'Examines raw sector blocks across the entire device. Detects file headers and chunk boundaries (JPEG, PNG, PDF, DOCX, ZIP, MP4) even on formatted or corrupted drives.'
                    : 'Examina blocos de setores brutos por todo o dispositivo. Detecta cabeçalhos e limites de arquivos (JPEG, PNG, PDF, DOCX, ZIP, MP4) mesmo em discos formatados ou corrompidos.'}
                </p>
                <div className="text-xs text-indigo-400 font-medium">
                  {language === 'en' ? '✓ Works on formatted or RAW partitions' : '✓ Funciona em partições formatadas ou em modo RAW'}
                </div>
              </div>
            </div>

            <div className="shrink-0 pt-4 border-t border-slate-800 flex items-center justify-between mt-auto">
              <button
                onClick={() => setCurrentStep(1)}
                className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800 transition"
              >
                {language === 'en' ? 'Back' : 'Voltar'}
              </button>
              <button
                onClick={() => setCurrentStep(3)}
                className="px-5 py-2.5 rounded-lg bg-sky-600 hover:bg-sky-500 text-xs font-semibold text-white shadow-md shadow-sky-600/25 transition flex items-center gap-1.5"
              >
                {language === 'en' ? 'Continue to Filters' : 'Continuar para Filtros'} <ArrowRight className="w-4 h-4" />
              </button>
            </div>
          </div>
        )}

        {/* STEP 3: Apply Filters */}
        {currentStep === 3 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-4xl mx-auto w-full">
            <h2 className="text-xl font-bold mb-1">
              {language === 'en' ? 'Step 3 — Apply Search Filters' : 'Etapa 3 — Aplicar Filtros de Busca'}
            </h2>
            <p className="text-xs text-slate-400 mb-6">
              {language === 'en'
                ? 'Select which file types you are looking to recover.'
                : 'Selecione quais tipos de arquivos você deseja recuperar.'}
            </p>

            <div className="grid grid-cols-3 gap-3 mb-8">
              {[
                { label: language === 'en' ? 'Images' : 'Imagens', ext: 'JPG, PNG, GIF, WebP, BMP', key: 'Image', icon: ImageIcon },
                { label: language === 'en' ? 'Documents' : 'Documentos', ext: 'PDF, DOCX, XLSX, PPTX', key: 'Document', icon: FileText },
                { label: language === 'en' ? 'Videos' : 'Vídeos', ext: 'MP4, MKV, MOV, AVI', key: 'Video', icon: Video },
                { label: language === 'en' ? 'Audio' : 'Áudio', ext: 'MP3, WAV, FLAC', key: 'Audio', icon: Music },
                { label: language === 'en' ? 'Archives' : 'Arquivos Compactados', ext: 'ZIP, RAR, 7Z, TAR', key: 'Archive', icon: Archive },
                { label: language === 'en' ? 'Code & Text' : 'Código & Texto', ext: 'TXT, JSON, MD, LOG', key: 'Text', icon: FileSearch },
              ].map((item) => {
                const isSelected = selectedCategories.includes(item.key);
                const Icon = item.icon;
                return (
                  <div
                    key={item.key}
                    onClick={() => {
                      if (isSelected) {
                        setSelectedCategories(selectedCategories.filter((c) => c !== item.key));
                      } else {
                        setSelectedCategories([...selectedCategories, item.key]);
                      }
                    }}
                    className={`p-4 rounded-lg border cursor-pointer transition flex items-center gap-3 ${
                      isSelected
                        ? 'border-sky-500 bg-sky-500/15 ring-2 ring-sky-500/30 text-sky-200'
                        : 'border-slate-800 bg-slate-900/40 text-slate-400 hover:border-slate-700'
                    }`}
                  >
                    <Icon className="w-5 h-5 text-sky-400" />
                    <div>
                      <div className="font-medium text-sm">{item.label}</div>
                      <div className="text-[11px] text-slate-500">{item.ext}</div>
                    </div>
                  </div>
                );
              })}
            </div>

            <div className="shrink-0 pt-4 border-t border-slate-800 flex items-center justify-between mt-auto">
              <button
                onClick={() => setCurrentStep(2)}
                className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800 transition"
              >
                {language === 'en' ? 'Back' : 'Voltar'}
              </button>
              <button
                onClick={handleStartScan}
                className="px-6 py-2.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-xs font-semibold text-white shadow-lg shadow-emerald-600/25 transition flex items-center gap-2"
              >
                <Play className="w-4 h-4 fill-current" />{' '}
                {language === 'en' ? 'Start Scan Now' : 'Iniciar Varredura Agora'}
              </button>
            </div>
          </div>
        )}

        {/* STEP 4: Scanning Screen / Preloader */}
        {currentStep === 4 && (
          <div className="flex-1 min-h-0 flex flex-col items-center justify-center max-w-2xl mx-auto w-full p-4">
            {isPreparingResults ? (
              <div className="w-full bg-slate-900 border border-sky-500/30 rounded-2xl p-8 text-center shadow-2xl animate-in fade-in zoom-in-95 duration-200">
                <div className="relative w-20 h-20 mx-auto mb-6 flex items-center justify-center">
                  <div className="absolute inset-0 rounded-full border-4 border-sky-500/20 border-t-sky-400 animate-spin" />
                  <div
                    className="absolute inset-2 rounded-full border-4 border-indigo-500/20 border-b-indigo-400 animate-spin"
                    style={{ animationDirection: 'reverse', animationDuration: '1.2s' }}
                  />
                  <Database className="w-8 h-8 text-sky-400 animate-pulse" />
                </div>
                <h2 className="text-2xl font-bold text-slate-100 mb-2">
                  {language === 'en' ? 'Structuring & Indexing Catalog...' : 'Estruturando e Indexando Catálogo...'}
                </h2>
                <p className="text-xs text-slate-400 mb-6 max-w-md mx-auto leading-relaxed">
                  {language === 'en'
                    ? `Cataloging ${preparingCount.toLocaleString()} recovered files, organizing partitions, and building high-performance view...`
                    : `Catalogando ${preparingCount.toLocaleString()} arquivos localizados, organizando limites de setores e preparando exibição de alta performance...`}
                </p>
                <div className="w-full bg-slate-800 h-2 rounded-full overflow-hidden shadow-inner">
                  <div className="bg-gradient-to-r from-sky-400 via-indigo-500 to-sky-400 h-full w-full animate-pulse" />
                </div>
              </div>
            ) : scanError ? (
              <div className="w-full bg-slate-900 border border-rose-500/40 rounded-2xl p-6 text-center shadow-xl">
                <div className="w-14 h-14 rounded-full bg-rose-500/10 text-rose-400 flex items-center justify-center mx-auto mb-4">
                  <AlertTriangle className="w-7 h-7" />
                </div>
                <h2 className="text-xl font-bold text-rose-300 mb-2">
                  {language === 'en' ? 'Scan Interrupted / Drive Access Error' : 'Erro no Acesso à Unidade / Varredura Interrompida'}
                </h2>
                <p className="text-xs text-slate-300 font-mono bg-black/40 p-3 rounded-lg border border-slate-800 mb-4 max-h-28 overflow-y-auto break-all">
                  {scanError}
                </p>
                <div className="text-xs text-slate-400 text-left bg-slate-950/60 p-3.5 rounded-lg border border-slate-800/80 mb-6 space-y-2">
                  <div className="font-semibold text-slate-300">
                    {language === 'en' ? 'Resolution & Troubleshooting:' : 'Como Resolver:'}
                  </div>
                  <div className="flex items-start gap-2">
                    <span className="text-sky-400 font-bold">•</span>
                    <span>
                      {language === 'en'
                        ? 'Administrator Elevation: Raw sector access to Windows logical drives (e.g. C:, D:) requires running OpenRe as Administrator. Right-click OpenRe.exe and choose "Run as Administrator".'
                        : 'Permissão de Administrador: O acesso direto a setores brutos de discos no Windows exige executar o OpenRe como Administrador. Clique com botão direito no OpenRe.exe e selecione "Executar como Administrador".'}
                    </span>
                  </div>
                  <div className="flex items-start gap-2">
                    <span className="text-sky-400 font-bold">•</span>
                    <span>
                      {language === 'en'
                        ? 'Un-elevated Mode: You can select a specific folder (e.g. Pictures, Documents, or disk image .img/.dd) to scan without requiring administrator privileges.'
                        : 'Modo sem Elevação: Você pode selecionar uma pasta específica (ex: Imagens, Documentos, ou arquivo de imagem .img/.dd) para varrer sem precisar de direitos de administrador.'}
                    </span>
                  </div>
                </div>

                <div className="flex items-center justify-center gap-3">
                  <button
                    onClick={() => {
                      setScanError(null);
                      setCurrentStep(1);
                    }}
                    className="px-5 py-2.5 rounded-lg border border-slate-700 hover:bg-slate-800 text-xs font-medium text-slate-200 transition flex items-center gap-2"
                  >
                    <ArrowLeft className="w-4 h-4" />
                    {language === 'en' ? 'Choose Different Storage' : 'Escolher Outro Armazenamento'}
                  </button>
                  <button
                    onClick={handleStartScan}
                    className="px-5 py-2.5 rounded-lg bg-sky-600 hover:bg-sky-500 text-xs font-semibold text-white shadow-md transition flex items-center gap-2"
                  >
                    <RotateCcw className="w-4 h-4" />
                    {language === 'en' ? 'Retry Scan' : 'Tentar Novamente'}
                  </button>
                </div>
              </div>
            ) : (
              <>
                <div className="w-16 h-16 rounded-full bg-sky-500/10 text-sky-400 flex items-center justify-center mb-6 animate-pulse">
                  <FileSearch className="w-8 h-8" />
                </div>

                <h2 className="text-2xl font-bold mb-2">
                  {language === 'en' ? 'Scanning in Progress' : 'Varredura em Andamento'}
                </h2>
                <p className="text-xs text-slate-400 mb-8">{scanProgress.currentLabel}</p>

                <div className="w-full bg-slate-800 h-3 rounded-full overflow-hidden mb-4">
                  <div
                    className="bg-gradient-to-r from-sky-500 to-indigo-500 h-full transition-all duration-300 ease-out"
                    style={{ width: `${scanProgress.percent}%` }}
                  />
                </div>

                <div className="w-full flex items-center justify-between text-xs text-slate-400 font-mono mb-8">
                  <span>{scanProgress.percent}%</span>
                  <span>{language === 'en' ? 'Speed:' : 'Velocidade:'} {scanProgress.speedMBps} MB/s</span>
                  <span>{language === 'en' ? 'Files Discovered' : 'Arquivos Encontrados'}: {scanProgress.filesCount}</span>
                </div>

                <button
                  onClick={handleCancelScan}
                  className="px-5 py-2 rounded-lg border border-rose-500/30 text-rose-400 hover:bg-rose-500/10 text-xs font-medium transition flex items-center gap-2"
                >
                  <Square className="w-3.5 h-3.5 fill-current" />
                  {language === 'en' ? 'Stop Scan' : 'Parar Varredura'}
                </button>
              </>
            )}
          </div>
        )}

        {/* STEP 5: Review Results */}
        {currentStep === 5 && (
          isPreparingResults ? (
            <div className="flex-1 min-h-0 flex flex-col items-center justify-center max-w-xl mx-auto w-full p-6 animate-in fade-in duration-200">
              <div className="w-full bg-slate-900 border border-sky-500/30 rounded-2xl p-8 text-center shadow-2xl">
                <div className="relative w-20 h-20 mx-auto mb-6 flex items-center justify-center">
                  <div className="absolute inset-0 rounded-full border-4 border-sky-500/20 border-t-sky-400 animate-spin" />
                  <div
                    className="absolute inset-2 rounded-full border-4 border-indigo-500/20 border-b-indigo-400 animate-spin"
                    style={{ animationDirection: 'reverse', animationDuration: '1.2s' }}
                  />
                  <Database className="w-8 h-8 text-sky-400 animate-pulse" />
                </div>
                <h2 className="text-2xl font-bold text-slate-100 mb-2">
                  {language === 'en' ? 'Structuring & Indexing Catalog...' : 'Estruturando & Indexando Catálogo...'}
                </h2>
                <p className="text-xs text-slate-400 mb-6 max-w-md mx-auto leading-relaxed">
                  {language === 'en'
                    ? `Building high-performance view and cataloging ${preparingCount.toLocaleString()} recovered items...`
                    : `Montando tabela de exibição e catalogando ${preparingCount.toLocaleString()} arquivos recuperados...`}
                </p>
                <div className="w-full bg-slate-800 h-2.5 rounded-full overflow-hidden shadow-inner mb-3">
                  <div className="bg-gradient-to-r from-sky-400 via-indigo-500 to-sky-400 h-full w-full animate-pulse" />
                </div>
                <div className="text-[11px] text-sky-400 font-mono animate-pulse">
                  {language === 'en' ? 'Preparing table rows & previews...' : 'Preparando linhas da tabela e prévias...'}
                </div>
              </div>
            </div>
          ) : (
          <div className={`flex-1 min-h-0 flex flex-col h-full overflow-hidden transition-all duration-150 ${previewFile ? 'mr-[490px]' : ''}`}>
            <div className="shrink-0 flex items-center justify-between gap-4 mb-3 pb-3 border-b border-slate-800">
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  placeholder={language === 'en' ? "Filter by name or extension..." : "Filtrar por nome ou extensão..."}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="px-3 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-sky-500 w-52 sm:w-60"
                />

                <select
                  value={filterCategory}
                  onChange={(e) => setFilterCategory(e.target.value)}
                  className="px-3 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs text-slate-200 focus:outline-none focus:border-sky-500"
                >
                  <option value="All">{language === 'en' ? 'All Categories' : 'Todas Categorias'}</option>
                  <option value="Image">{language === 'en' ? 'Images' : 'Imagens'}</option>
                  <option value="Document">{language === 'en' ? 'Documents' : 'Documentos'}</option>
                  <option value="Video">{language === 'en' ? 'Videos' : 'Vídeos'}</option>
                  <option value="Audio">{language === 'en' ? 'Audio' : 'Áudio'}</option>
                  <option value="Archive">{language === 'en' ? 'Archives' : 'Arquivos Compactados'}</option>
                  <option value="Text">{language === 'en' ? 'Text' : 'Texto'}</option>
                </select>

                <div className="flex items-center gap-1.5 ml-1">
                  <button
                    onClick={toggleSelectPage}
                    className="px-2 py-1 rounded border border-slate-800 bg-slate-900 hover:bg-slate-800 text-[11px] text-slate-300 transition"
                  >
                    {isPageFullySelected
                      ? (language === 'en' ? 'Deselect Page' : 'Desmarcar Pág')
                      : (language === 'en' ? 'Select Page' : 'Marcar Pág')}
                  </button>
                  <button
                    onClick={toggleSelectAllFiltered}
                    className="px-2 py-1 rounded border border-slate-800 bg-slate-900 hover:bg-slate-800 text-[11px] text-slate-300 transition"
                  >
                    {isAllFullySelected
                      ? (language === 'en' ? 'Deselect All' : 'Desmarcar Todos')
                      : (language === 'en' ? 'Select All' : 'Marcar Todos')}
                  </button>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <div className="text-xs text-slate-400 font-mono hidden md:block">
                  <strong className="text-sky-400">{selectedFileIds.size}</strong> {language === 'en' ? 'of' : 'de'} {filteredFiles.length}
                </div>

                {/* View Mode Toggle */}
                <div className="flex items-center border border-slate-800 rounded-lg overflow-hidden">
                  <button
                    onClick={() => setViewMode('list')}
                    className={`p-1.5 ${viewMode === 'list' ? 'bg-slate-800 text-sky-400' : 'text-slate-400 hover:text-slate-200'}`}
                    title={language === 'en' ? 'List View' : 'Visualização em Lista'}
                  >
                    <List className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => setViewMode('grid')}
                    className={`p-1.5 ${viewMode === 'grid' ? 'bg-slate-800 text-sky-400' : 'text-slate-400 hover:text-slate-200'}`}
                    title={language === 'en' ? 'Grid View' : 'Visualização em Grade'}
                  >
                    <LayoutGrid className="w-4 h-4" />
                  </button>
                </div>

                <button
                  onClick={() => {
                    setPreviewFile(null);
                    setCurrentStep(6);
                  }}
                  disabled={selectedFileIds.size === 0}
                  className="px-4 py-1.5 rounded-lg bg-sky-600 hover:bg-sky-500 disabled:opacity-50 text-xs font-semibold text-white shadow-md shadow-sky-600/25 transition flex items-center gap-1.5 shrink-0"
                >
                  {language === 'en' ? 'Proceed to Recover' : 'Avançar para Recuperação'} ({selectedFileIds.size}) <ArrowRight className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            <div className="flex-1 min-h-0 overflow-y-auto pr-1">
              {paginatedFiles.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-center p-8 text-slate-500">
                  <FileSearch className="w-12 h-12 text-slate-600 mb-3" />
                  <p className="text-sm font-medium text-slate-300 mb-1">
                    {language === 'en' ? 'No files match your filter' : 'Nenhum arquivo corresponde ao filtro'}
                  </p>
                  <p className="text-xs text-slate-500">
                    {language === 'en'
                      ? 'Try clearing the search query or selecting "All Categories".'
                      : 'Tente limpar a busca ou selecionar "Todas Categorias".'}
                  </p>
                </div>
              ) : viewMode === 'list' ? (
                <table className="w-full text-left text-xs">
                  <thead className="sticky top-0 bg-slate-900 text-slate-400 font-semibold border-b border-slate-800 z-10">
                    <tr>
                      <th className="py-2 px-3 w-8">
                        <input
                          type="checkbox"
                          checked={isPageFullySelected}
                          onChange={toggleSelectPage}
                          title={language === 'en' ? "Select/deselect page" : "Selecionar/desmarcar página"}
                        />
                      </th>
                      <th className="py-2 px-3">{language === 'en' ? 'Filename' : 'Nome do Arquivo'}</th>
                      <th className="py-2 px-3">{language === 'en' ? 'Category' : 'Categoria'}</th>
                      <th className="py-2 px-3">{language === 'en' ? 'Size' : 'Tamanho'}</th>
                      <th className="py-2 px-3">{language === 'en' ? 'Integrity Rating' : 'Integridade'}</th>
                      <th className="py-2 px-3 text-right">{language === 'en' ? 'Actions' : 'Ações'}</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-850">
                    {paginatedFiles.map((file) => {
                      const isSelected = selectedFileIds.has(file.id);
                      const isCurrentPreview = previewFile?.id === file.id;

                      return (
                        <tr
                          key={file.id}
                          onClick={() => openPreview(file)}
                          className={`transition cursor-pointer group select-none ${
                            isCurrentPreview
                              ? 'bg-sky-500/15 border-l-2 border-sky-400'
                              : isSelected
                              ? 'bg-slate-900/80 hover:bg-slate-850'
                              : 'hover:bg-slate-900/60'
                          }`}
                        >
                          <td
                            className="py-2 px-3"
                            onClick={(e) => e.stopPropagation()}
                          >
                            <input
                              type="checkbox"
                              checked={isSelected}
                              onChange={(e) => {
                                const next = new Set(selectedFileIds);
                                if (e.target.checked) next.add(file.id);
                                else next.delete(file.id);
                                setSelectedFileIds(next);
                              }}
                              className="cursor-pointer"
                            />
                          </td>
                          <td className="py-2 px-3 flex items-center gap-2 font-mono">
                            {file.category === 'Image' && imageThumbnails[file.id] ? (
                              <img
                                src={imageThumbnails[file.id]}
                                alt={file.filename}
                                className="w-5 h-5 object-cover rounded border border-slate-700/80 shrink-0 select-none"
                                loading="lazy"
                              />
                            ) : (
                              getCategoryIcon(file.category)
                            )}
                            <span className={`font-medium truncate max-w-xs ${isCurrentPreview ? 'text-sky-300 font-semibold' : 'text-slate-200'}`}>
                              {file.filename}
                            </span>
                          </td>
                          <td className="py-2 px-3 text-slate-400">{getCategoryName(file.category)}</td>
                          <td className="py-2 px-3 text-slate-400 font-mono">
                            {(file.size_bytes / 1024).toFixed(1)} KB
                          </td>
                          <td className="py-2 px-3">{getIntegrityBadge(file.integrity)}</td>
                          <td className="py-2 px-3 text-right">
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                openPreview(file);
                              }}
                              className={`px-2 py-1 rounded text-xs transition inline-flex items-center gap-1 ${
                                isCurrentPreview
                                  ? 'bg-sky-600 text-white font-medium shadow-sm'
                                  : 'bg-slate-800 hover:bg-slate-700 text-slate-200'
                              }`}
                            >
                              <Eye className="w-3.5 h-3.5" />
                              <span>{language === 'en' ? 'Preview' : 'Visualizar'}</span>
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              ) : (
                <div className={`grid gap-3 p-1 ${previewFile ? 'grid-cols-2 md:grid-cols-3' : 'grid-cols-2 md:grid-cols-4'}`}>
                  {paginatedFiles.map((file) => {
                    const isSelected = selectedFileIds.has(file.id);
                    const isCurrentPreview = previewFile?.id === file.id;

                    return (
                      <div
                        key={file.id}
                        onClick={() => openPreview(file)}
                        className={`p-3 rounded-xl border transition flex flex-col justify-between cursor-pointer group select-none ${
                          isCurrentPreview
                            ? 'border-sky-400 bg-sky-500/15 shadow-md shadow-sky-500/10 ring-1 ring-sky-400/50'
                            : isSelected
                            ? 'border-sky-500/50 bg-sky-500/10'
                            : 'border-slate-800 bg-slate-900/40 hover:border-slate-700 hover:bg-slate-900/70'
                        }`}
                      >
                        <div className="flex items-center justify-between mb-2">
                          <div
                            className="flex items-center gap-2"
                            onClick={(e) => e.stopPropagation()}
                          >
                            <input
                              type="checkbox"
                              checked={isSelected}
                              onChange={(e) => {
                                const next = new Set(selectedFileIds);
                                if (e.target.checked) next.add(file.id);
                                else next.delete(file.id);
                                setSelectedFileIds(next);
                              }}
                              className="cursor-pointer"
                            />
                            {getCategoryIcon(file.category)}
                          </div>
                          {getIntegrityBadge(file.integrity)}
                        </div>

                        {/* Content Quick Glance Area in Grid Card */}
                        <div className="h-16 rounded-lg bg-black/40 border border-slate-800/80 mb-2 flex items-center justify-center p-1.5 text-center overflow-hidden">
                          {file.category === 'Image' ? (
                            imageThumbnails[file.id] ? (
                              <img
                                src={imageThumbnails[file.id]}
                                alt={file.filename}
                                className="max-h-full max-w-full object-contain rounded select-none group-hover:scale-105 transition-transform duration-200"
                                loading="lazy"
                              />
                            ) : (
                              <div className="flex flex-col items-center text-slate-400 group-hover:text-sky-300 transition-colors">
                                <ImageIcon className="w-6 h-6 mb-1 opacity-80" />
                                <span className="text-[10px] font-mono uppercase">{file.extension} {language === 'en' ? 'Image' : 'Imagem'}</span>
                              </div>
                            )
                          ) : file.category === 'Audio' ? (
                            <div className="flex flex-col items-center text-amber-400/80 group-hover:text-amber-300 transition-colors">
                              <Volume2 className="w-6 h-6 mb-1 opacity-80" />
                              <span className="text-[10px] font-mono uppercase">{file.extension} {language === 'en' ? 'Audio' : 'Áudio'}</span>
                            </div>
                          ) : file.category === 'Video' ? (
                            <div className="flex flex-col items-center text-purple-400/80 group-hover:text-purple-300 transition-colors">
                              <Video className="w-6 h-6 mb-1 opacity-80" />
                              <span className="text-[10px] font-mono uppercase">{file.extension} {language === 'en' ? 'Stream' : 'Vídeo'}</span>
                            </div>
                          ) : (
                            <div className="flex flex-col items-center text-slate-500 group-hover:text-slate-300 transition-colors">
                              <FileText className="w-6 h-6 mb-1 opacity-80" />
                              <span className="text-[10px] font-mono uppercase">{file.extension || (language === 'en' ? 'Data' : 'Dados')}</span>
                            </div>
                          )}
                        </div>

                        <div className={`font-mono text-xs truncate mb-1 ${isCurrentPreview ? 'text-sky-300 font-bold' : 'text-slate-200 font-medium'}`}>
                          {file.filename}
                        </div>
                        <div className="flex items-center justify-between text-[11px] text-slate-400 font-mono">
                          <span>{(file.size_bytes / 1024).toFixed(1)} KB</span>
                          <span className={`transition ${isCurrentPreview ? 'text-sky-300 font-semibold underline' : 'text-sky-400 group-hover:underline'}`}>
                            {language === 'en' ? 'Preview' : 'Ver'}
                          </span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Pagination Controls Bar */}
            <div className="shrink-0 pt-3 mt-2 border-t border-slate-800 flex flex-wrap items-center justify-between gap-3 select-none text-xs">
              {/* Left: Showing range */}
              <div className="text-slate-400 font-mono text-[11px]">
                {totalItems > 0 ? (
                  <span>
                    {language === 'en' ? 'Showing' : 'Exibindo'}{' '}
                    <strong className="text-slate-200">{startIndex + 1}</strong>–
                    <strong className="text-slate-200">{endIndex}</strong> {language === 'en' ? 'of' : 'de'}{' '}
                    <strong className="text-sky-400">{totalItems}</strong> {language === 'en' ? 'files' : 'arquivos'}
                  </span>
                ) : (
                  <span>{language === 'en' ? 'No files found' : 'Nenhum arquivo encontrado'}</span>
                )}
              </div>

              {/* Center: Page size buttons */}
              <div className="flex items-center gap-1.5">
                <span className="text-slate-500 text-[11px] mr-1">
                  {language === 'en' ? 'Per page:' : 'Por página:'}
                </span>
                {[20, 50, 100, 200, 300].map((size) => (
                  <button
                    key={size}
                    onClick={() => setPageSize(size)}
                    className={`px-2 py-0.5 rounded text-[11px] font-mono transition ${
                      pageSize === size
                        ? 'bg-sky-600 text-white font-bold shadow-sm'
                        : 'bg-slate-900 border border-slate-800 text-slate-400 hover:text-slate-200 hover:border-slate-700'
                    }`}
                  >
                    {size}
                  </button>
                ))}
              </div>

              {/* Right: Navigation controls */}
              <div className="flex items-center gap-1">
                <button
                  onClick={() => setCurrentPage(1)}
                  disabled={safeCurrentPage <= 1}
                  title={language === 'en' ? 'First page' : 'Primeira página'}
                  className="p-1.5 rounded border border-slate-800 bg-slate-900 hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed text-slate-300"
                >
                  <ChevronsLeft className="w-3.5 h-3.5" />
                </button>
                <button
                  onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                  disabled={safeCurrentPage <= 1}
                  title={language === 'en' ? 'Previous page' : 'Página anterior'}
                  className="p-1.5 rounded border border-slate-800 bg-slate-900 hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed text-slate-300"
                >
                  <ChevronLeft className="w-3.5 h-3.5" />
                </button>

                <span className="px-2 text-[11px] text-slate-300 font-mono">
                  {language === 'en' ? 'Page' : 'Pág'}{' '}
                  <strong className="text-sky-400">{safeCurrentPage}</strong> / {totalPages}
                </span>

                <button
                  onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                  disabled={safeCurrentPage >= totalPages}
                  title={language === 'en' ? 'Next page' : 'Próxima página'}
                  className="p-1.5 rounded border border-slate-800 bg-slate-900 hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed text-slate-300"
                >
                  <ChevronRight className="w-3.5 h-3.5" />
                </button>
                <button
                  onClick={() => setCurrentPage(totalPages)}
                  disabled={safeCurrentPage >= totalPages}
                  title={language === 'en' ? 'Last page' : 'Última página'}
                  className="p-1.5 rounded border border-slate-800 bg-slate-900 hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed text-slate-300"
                >
                  <ChevronsRight className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          </div>
          )
        )}

        {/* STEP 6: Recover Destination */}
        {currentStep === 6 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-2xl mx-auto w-full justify-center">
            <h2 className="text-2xl font-bold mb-2">
              {language === 'en' ? 'Step 6 — Choose Destination Directory' : 'Etapa 6 — Escolher Diretório de Destino'}
            </h2>
            <p className="text-xs text-slate-400 mb-6">
              {language === 'en'
                ? 'Select a safe location to write the recovered files.'
                : 'Selecione um local seguro para gravar os arquivos recuperados.'}
            </p>

            <div className="p-4 rounded-xl border border-slate-800 bg-slate-900/60 mb-4">
              <label className="text-xs font-semibold text-slate-300 block mb-2">
                {language === 'en' ? 'Recovery Target Directory:' : 'Diretório de Destino da Recuperação:'}
              </label>
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  readOnly
                  placeholder={
                    language === 'en'
                      ? 'Click Browse to choose destination...'
                      : 'Clique em Procurar para escolher o destino...'
                  }
                  value={destPath}
                  className="flex-1 px-3 py-2 rounded-lg bg-slate-950 border border-slate-800 text-xs text-slate-200 font-mono focus:outline-none"
                />
                <button
                  onClick={handleSelectDestFolder}
                  className="px-4 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200 transition flex items-center gap-1.5"
                >
                  <FolderOpen className="w-4 h-4" /> {language === 'en' ? 'Browse' : 'Procurar'}
                </button>
              </div>
            </div>

            {collisionWarning ? (
              <div className="p-4 rounded-xl border border-rose-500/40 bg-rose-500/10 text-rose-300 text-xs flex items-start gap-3 mb-6">
                <AlertTriangle className="w-5 h-5 text-rose-400 shrink-0 mt-0.5" />
                <div>
                  <div className="font-semibold mb-1">
                    {language === 'en' ? 'Overwriting Risk Detected!' : 'Risco de Sobrescrita Detectado!'}
                  </div>
                  <div>{collisionWarning}</div>
                </div>
              </div>
            ) : (
              <div className="p-3.5 rounded-lg border border-slate-800 bg-slate-900/40 text-slate-400 text-xs flex items-center gap-2 mb-6">
                <ShieldCheck className="w-4 h-4 text-emerald-400" />
                <span>
                  {language === 'en'
                    ? 'Files will be written with SHA-256 validation. Existing files are never overwritten.'
                    : 'Os arquivos serão gravados com validação SHA-256. Arquivos existentes nunca são sobrescritos.'}
                </span>
              </div>
            )}

            <div className="shrink-0 pt-4 border-t border-slate-800 flex items-center justify-between">
              <button
                onClick={() => setCurrentStep(5)}
                className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800 transition"
              >
                {language === 'en' ? 'Back to Results' : 'Voltar aos Resultados'}
              </button>
              <button
                disabled={!destPath || (!!collisionWarning && enforceSeparation) || isRecovering}
                onClick={handleStartRecovery}
                className="px-6 py-2.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-xs font-semibold text-white shadow-lg shadow-emerald-600/25 transition flex items-center gap-2"
              >
                <Download className="w-4 h-4" />{' '}
                {language === 'en' ? 'Start Recovery Process' : 'Iniciar Processo de Recuperação'}
              </button>
            </div>
          </div>
        )}

        {/* STEP 7: Audit Report Screen */}
        {currentStep === 7 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-3xl mx-auto w-full">
            <div className="text-center mb-6 shrink-0">
              <div className="w-12 h-12 rounded-full bg-emerald-500/20 text-emerald-400 flex items-center justify-center mx-auto mb-3">
                <CheckCircle2 className="w-6 h-6" />
              </div>
              <h2 className="text-2xl font-bold">
                {language === 'en' ? 'Recovery Completed Successfully' : 'Recuperação Concluída com Sucesso'}
              </h2>
              <p className="text-xs text-slate-400">
                {language === 'en'
                  ? 'All chosen files have been written to the destination directory.'
                  : 'Todos os arquivos selecionados foram gravados no diretório de destino.'}
              </p>
            </div>

            <div className="flex-1 min-h-0 bg-slate-900 rounded-xl border border-slate-800 p-4 font-mono text-xs overflow-y-auto mb-6 whitespace-pre-wrap text-slate-300">
              {recoveryReportMd}
            </div>

            <div className="shrink-0 flex items-center justify-between pt-4 border-t border-slate-800">
              <button
                onClick={() => setCurrentStep(0)}
                className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800 transition flex items-center gap-1.5"
              >
                <RotateCcw className="w-3.5 h-3.5" />{' '}
                {language === 'en' ? 'Start New Recovery' : 'Iniciar Nova Recuperação'}
              </button>

              <button
                onClick={() => sendIpc('open_output_folder', { path: destPath })}
                className="px-5 py-2 rounded-lg bg-sky-600 hover:bg-sky-500 text-xs font-semibold text-white shadow-md shadow-sky-600/25 transition flex items-center gap-1.5"
              >
                <FolderOpen className="w-4 h-4" />{' '}
                {language === 'en' ? 'Open Destination Folder' : 'Abrir Pasta de Destino'}
              </button>
            </div>
          </div>
        )}
      </main>

      {/* Docked Footer Diagnostic Terminal */}
      {showTerminal && (
        <div className="shrink-0 border-t border-slate-800 bg-slate-950 flex flex-col shadow-2xl z-30 select-none">
          {/* Vertical Resize Drag Handle */}
          <div
            onMouseDown={(e) => {
              e.preventDefault();
              setIsDraggingTerminal(true);
            }}
            className={`h-2 w-full flex items-center justify-center cursor-row-resize transition-colors ${
              isDraggingTerminal ? 'bg-sky-500' : 'bg-slate-900/90 hover:bg-sky-500/70'
            }`}
            title={language === 'en' ? 'Drag up or down to resize terminal' : 'Arraste para redimensionar o terminal'}
          >
            <div className="w-12 h-1 rounded-full bg-slate-600 hover:bg-white transition-colors" />
          </div>

          {/* Terminal Titlebar */}
          <div className="h-8 px-4 bg-slate-900/90 border-b border-slate-800/80 flex items-center justify-between text-xs">
            <div className="flex items-center gap-2.5">
              <span className="flex h-2 w-2 relative">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
              </span>
              <span className="font-mono font-semibold text-slate-300 flex items-center gap-1.5">
                <Terminal className="w-3.5 h-3.5 text-emerald-400" />
                {language === 'en' ? 'Diagnostic Terminal & System Logs' : 'Terminal de Diagnóstico & Logs do Sistema'}
              </span>
              <span className="px-1.5 py-0.2 rounded bg-slate-800 text-[10px] font-mono text-slate-400">
                {terminalLogs.length} {language === 'en' ? 'events' : 'eventos'}
              </span>
              <span className="text-[10px] font-mono text-slate-500 hidden sm:inline">
                ({terminalHeight}px)
              </span>
            </div>

            <div className="flex items-center gap-1.5">
              <button
                onClick={() => {
                  const text = terminalLogs.map((l) => `[${l.timestamp}] [${l.level.toUpperCase()}] ${l.message}`).join('\n');
                  navigator.clipboard?.writeText(text);
                  appendLog('info', 'All terminal logs copied to clipboard.');
                }}
                className="px-2 py-0.5 rounded text-[11px] font-mono text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition flex items-center gap-1"
                title={language === 'en' ? 'Copy logs to clipboard' : 'Copiar logs para a área de transferência'}
              >
                <Copy className="w-3 h-3" />
                <span>{language === 'en' ? 'Copy' : 'Copiar'}</span>
              </button>

              <button
                onClick={() => setTerminalLogs([])}
                className="px-2 py-0.5 rounded text-[11px] font-mono text-slate-400 hover:text-rose-400 hover:bg-slate-800 transition flex items-center gap-1"
                title={language === 'en' ? 'Clear terminal output' : 'Limpar terminal'}
              >
                <Trash2 className="w-3 h-3" />
                <span>{language === 'en' ? 'Clear' : 'Limpar'}</span>
              </button>

              <button
                onClick={() => setShowTerminal(false)}
                className="p-1 rounded text-slate-400 hover:text-rose-400 hover:bg-slate-800 transition ml-1"
                title={language === 'en' ? 'Close terminal panel (can be re-opened in Header or Settings)' : 'Fechar terminal (pode ser reaberto no Cabeçalho ou Configurações)'}
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          {/* Terminal Console Output */}
          <div
            style={{ height: `${terminalHeight}px` }}
            className="overflow-y-auto p-2.5 font-mono text-[11px] space-y-1 select-text bg-black/80"
          >
            {terminalLogs.length === 0 ? (
              <p className="text-slate-500 italic text-center py-4">
                {language === 'en' ? 'No log events. System ready.' : 'Nenhum evento registrado. Sistema pronto.'}
              </p>
            ) : (
              terminalLogs.map((log) => {
                let color = 'text-sky-300';
                let badge = 'bg-sky-950 text-sky-400 border border-sky-800/60';
                if (log.level === 'success') {
                  color = 'text-emerald-300';
                  badge = 'bg-emerald-950 text-emerald-400 border border-emerald-800/60';
                } else if (log.level === 'warn') {
                  color = 'text-amber-300';
                  badge = 'bg-amber-950 text-amber-400 border border-amber-800/60';
                } else if (log.level === 'error') {
                  color = 'text-rose-300 font-semibold';
                  badge = 'bg-rose-950 text-rose-400 border border-rose-800/60';
                }
                return (
                  <div key={log.id} className="flex items-start gap-2 leading-relaxed hover:bg-white/[0.03] px-1 py-0.5 rounded">
                    <span className="text-slate-500 shrink-0 select-none">[{log.timestamp}]</span>
                    <span className={`px-1.5 py-0.2 text-[9px] uppercase rounded font-bold shrink-0 select-none ${badge}`}>
                      {log.level}
                    </span>
                    <span className={`break-all ${color}`}>{log.message}</span>
                  </div>
                );
              })
            )}
            <div ref={terminalEndRef} />
          </div>
        </div>
      )}

      {/* Slide-out Preview Drawer */}
      {previewFile && !isPreparingResults && (
        <div
          className="fixed right-0 w-[480px] bg-slate-900 border-l border-slate-800 shadow-2xl flex flex-col z-40 top-14 transition-all duration-150 animate-in slide-in-from-right"
          style={{ bottom: showTerminal ? `${terminalHeight + 42}px` : 0 }}
        >
          {/* Header */}
          <div className="p-4 border-b border-slate-800 flex items-center justify-between">
            <div className="font-semibold text-sm flex items-center gap-2 truncate">
              {getCategoryIcon(previewFile.category)}
              <span className="truncate">{previewFile.filename}</span>
            </div>
            <button
              onClick={() => { setPreviewFile(null); setPreviewData(null); }}
              className="p-1 rounded text-slate-400 hover:text-slate-100 hover:bg-slate-800 transition"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Mode Switcher Tabs */}
          <div className="px-4 pt-2.5 pb-2 border-b border-slate-800 flex items-center gap-2 bg-slate-950/40 select-none">
            <button
              onClick={() => setPreviewTab('media')}
              className={`px-3 py-1 rounded-md text-xs font-semibold transition flex items-center gap-1.5 ${
                previewTab === 'media'
                  ? 'bg-sky-600 text-white shadow-sm'
                  : 'bg-slate-900 border border-slate-800 text-slate-400 hover:text-slate-200'
              }`}
            >
              <Eye className="w-3.5 h-3.5" />
              {language === 'en' ? 'Visual / Media Preview' : 'Visualização Real'}
            </button>
            <button
              onClick={() => setPreviewTab('hex')}
              className={`px-3 py-1 rounded-md text-xs font-semibold transition flex items-center gap-1.5 ${
                previewTab === 'hex'
                  ? 'bg-sky-600 text-white shadow-sm'
                  : 'bg-slate-900 border border-slate-800 text-slate-400 hover:text-slate-200'
              }`}
            >
              <FileCode className="w-3.5 h-3.5" />
              {language === 'en' ? 'Bytecode Hex Dump' : 'Inspeção Hexadecimal'}
            </button>
          </div>

          {/* Drawer Body */}
          <div className="flex-1 min-h-0 overflow-y-auto p-4 space-y-4">
            {previewTab === 'media' ? (
              <>
                {/* Image Media Preview */}
                {(previewFile.category === 'Image' ||
                  ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp'].includes(previewFile.extension.toLowerCase())) && (
                  <div className="border border-slate-800 rounded-xl p-3 bg-slate-950 flex flex-col items-center">
                    {!previewData ? (
                      <div className="py-12 text-center text-slate-400 flex flex-col items-center">
                        <RotateCcw className="w-8 h-8 animate-spin text-sky-400 mb-2" />
                        <p className="text-xs font-medium">
                          {language === 'en' ? 'Loading image stream...' : 'Carregando imagem...'}
                        </p>
                      </div>
                    ) : (previewData.media_data_url || previewData.thumbnail_base64) ? (
                      <div className="w-full flex flex-col items-center">
                        <img
                          src={previewData.media_data_url || previewData.thumbnail_base64}
                          alt={previewFile.filename}
                          className="max-h-64 max-w-full object-contain rounded-lg shadow-lg border border-slate-850"
                        />
                        {previewData.dimensions && (
                          <div className="text-[11px] text-slate-400 mt-2.5 font-mono flex items-center gap-2">
                            <span className="px-2 py-0.5 rounded bg-slate-900 border border-slate-800">
                              {previewData.dimensions[0]} × {previewData.dimensions[1]} px
                            </span>
                            <span className="text-slate-500">•</span>
                            <span className="text-emerald-400 font-semibold">{previewFile.extension.toUpperCase()}</span>
                          </div>
                        )}
                      </div>
                    ) : (
                      <div className="py-8 text-center text-slate-500">
                        <ImageIcon className="w-10 h-10 mx-auto mb-2 text-slate-600" />
                        <p className="text-xs">
                          {language === 'en' ? 'No preview image available.' : 'Nenhuma imagem disponível.'}
                        </p>
                      </div>
                    )}
                  </div>
                )}

                {/* Audio Media Preview */}
                {(previewFile.category === 'Audio' ||
                  ['mp3', 'wav', 'ogg', 'flac', 'm4a', 'aac', 'wma'].includes(previewFile.extension.toLowerCase())) && (
                  <div className="border border-slate-800 rounded-xl p-4 bg-slate-950 flex flex-col items-center">
                    <div className="w-16 h-16 rounded-2xl bg-amber-500/10 border border-amber-500/20 text-amber-400 flex items-center justify-center mb-3 shadow-inner">
                      <Volume2 className="w-8 h-8" />
                    </div>
                    <div className="text-xs font-semibold text-slate-200 mb-1">{previewFile.filename}</div>
                    <div className="text-[11px] text-slate-400 mb-3 font-mono">
                      {previewFile.extension.toUpperCase()} {language === 'en' ? 'Audio' : 'Áudio'} • {(previewFile.size_bytes / (1024 * 1024)).toFixed(2)} MB
                    </div>
                    {!previewData ? (
                      <div className="py-4 text-center text-slate-400 flex flex-col items-center">
                        <RotateCcw className="w-6 h-6 animate-spin text-amber-400 mb-2" />
                        <p className="text-xs">{language === 'en' ? 'Loading audio buffer...' : 'Carregando áudio...'}</p>
                      </div>
                    ) : previewData.media_data_url ? (
                      <div className="w-full">
                        <audio controls className="w-full rounded-lg" src={previewData.media_data_url} preload="auto" />
                        <p className="text-[10px] text-slate-500 text-center mt-2">
                          {language === 'en'
                            ? 'Audio stream loaded directly from carved raw sectors.'
                            : 'Fluxo de áudio carregado diretamente dos setores gravados.'}
                        </p>
                      </div>
                    ) : (
                      <p className="text-xs text-amber-400/80 italic">
                        {language === 'en' ? 'Audio stream buffer unavailable.' : 'Buffer de áudio indisponível.'}
                      </p>
                    )}
                  </div>
                )}

                {/* Video Media Preview */}
                {(previewFile.category === 'Video' ||
                  ['mp4', 'webm', 'mkv', 'avi', 'mov'].includes(previewFile.extension.toLowerCase())) && (
                  <div className="border border-slate-800 rounded-xl p-3 bg-slate-950 flex flex-col items-center">
                    {!previewData ? (
                      <div className="py-12 text-center text-slate-400 flex flex-col items-center">
                        <RotateCcw className="w-8 h-8 animate-spin text-purple-400 mb-2" />
                        <p className="text-xs font-medium">
                          {language === 'en' ? 'Buffering video stream...' : 'Carregando vídeo...'}
                        </p>
                      </div>
                    ) : previewData.media_data_url ? (
                      <div className="w-full flex flex-col items-center">
                        <video
                          controls
                          className="w-full max-h-64 rounded-lg bg-black border border-slate-800 object-contain shadow-lg"
                          src={previewData.media_data_url}
                          preload="metadata"
                        />
                        <div className="w-full flex justify-between items-center text-[10px] text-slate-400 mt-2 font-mono px-1">
                          <span className="text-purple-400 font-semibold">{previewFile.extension.toUpperCase()} {language === 'en' ? 'Video' : 'Vídeo'}</span>
                          <span>{(previewFile.size_bytes / (1024 * 1024)).toFixed(2)} MB</span>
                        </div>
                      </div>
                    ) : (
                      <div className="py-8 text-center text-slate-500">
                        <Video className="w-10 h-10 mx-auto mb-2 text-slate-600" />
                        <p className="text-xs">
                          {language === 'en' ? 'Video preview could not be streamed.' : 'Prévia de vídeo não pôde ser reproduzida.'}
                        </p>
                      </div>
                    )}
                  </div>
                )}

                {/* Text Snippet Preview */}
                {previewData?.text_snippet && (
                  <div className="border border-slate-800 rounded-lg p-3 bg-slate-950">
                    <div className="text-xs font-semibold text-slate-400 mb-2 flex items-center gap-1.5">
                      <FileCode className="w-3.5 h-3.5 text-sky-400" />
                      {language === 'en' ? 'Decoded Text Content:' : 'Conteúdo de Texto Decodificado:'}
                    </div>
                    <pre className="text-[11px] font-mono text-slate-200 whitespace-pre-wrap max-h-56 overflow-y-auto p-2.5 bg-slate-900 rounded border border-slate-850">
                      {previewData.text_snippet}
                    </pre>
                  </div>
                )}

                {/* Single-File Instant Restore & Save (Available for ALL file types) */}
                <div className="p-3 rounded-xl border border-indigo-500/30 bg-indigo-500/10 flex items-center justify-between shadow-sm">
                  <div className="pr-2">
                    <div className="font-semibold text-xs text-indigo-200 flex items-center gap-1.5 mb-0.5">
                      {['jpg', 'jpeg', 'png'].includes(previewFile.extension.toLowerCase()) ? (
                        <>
                          <Wrench className="w-3.5 h-3.5 text-indigo-400" />
                          <span>{language === 'en' ? 'Restore & Repair File' : 'Restaurar & Reparar Arquivo'}</span>
                        </>
                      ) : (
                        <>
                          <Download className="w-3.5 h-3.5 text-sky-400" />
                          <span>{language === 'en' ? 'Restore & Save Copy' : 'Restaurar & Salvar Cópia'}</span>
                        </>
                      )}
                    </div>
                    <div className="text-[11px] text-slate-400 leading-tight">
                      {['jpg', 'jpeg', 'png'].includes(previewFile.extension.toLowerCase())
                        ? (language === 'en'
                            ? 'Reconstruct missing markers and choose directory to save.'
                            : 'Reconstrói marcadores perdidos e permite escolher onde salvar.')
                        : (language === 'en'
                            ? 'Extract this file immediately and choose destination path.'
                            : 'Extrai este arquivo imediatamente e permite escolher onde salvar.')}
                    </div>
                  </div>
                  <button
                    onClick={handleRepairCopy}
                    className="px-3.5 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-xs font-semibold text-white transition flex items-center gap-1.5 shrink-0 shadow-md shadow-indigo-600/20"
                    title={language === 'en' ? 'Choose where to save this file copy' : 'Escolher onde salvar a cópia deste arquivo'}
                  >
                    {['jpg', 'jpeg', 'png'].includes(previewFile.extension.toLowerCase()) ? (
                      <>
                        <Wrench className="w-3.5 h-3.5" />
                        <span>{language === 'en' ? 'Repair & Save' : 'Reparar & Salvar'}</span>
                      </>
                    ) : (
                      <>
                        <FolderCheck className="w-3.5 h-3.5" />
                        <span>{language === 'en' ? 'Save File...' : 'Salvar Arquivo...'}</span>
                      </>
                    )}
                  </button>
                </div>

                {repairReport && (
                  <div
                    className={`p-3.5 rounded-xl border text-xs space-y-2.5 ${
                      repairReport.saved
                        ? 'border-emerald-500/40 bg-emerald-500/10 text-emerald-200'
                        : repairReport.user_cancelled
                        ? 'border-amber-500/40 bg-amber-500/10 text-amber-200'
                        : repairReport.success
                        ? 'border-indigo-500/40 bg-indigo-500/10 text-indigo-200'
                        : 'border-rose-500/40 bg-rose-500/10 text-rose-200'
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <div className="font-bold flex items-center gap-1.5 text-xs">
                        {repairReport.saved ? (
                          <>
                            <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                            <span>{language === 'en' ? 'File Restored & Saved Successfully!' : 'Arquivo Restaurado & Salvo com Sucesso!'}</span>
                          </>
                        ) : repairReport.user_cancelled ? (
                          <>
                            <AlertTriangle className="w-4 h-4 text-amber-400" />
                            <span>{language === 'en' ? 'Save Cancelled by User' : 'Salvamento Cancelado pelo Usuário'}</span>
                          </>
                        ) : repairReport.success ? (
                          <>
                            <CheckCircle2 className="w-4 h-4 text-indigo-400" />
                            <span>{language === 'en' ? 'Extraction Completed' : 'Extração Concluída'}</span>
                          </>
                        ) : (
                          <>
                            <AlertTriangle className="w-4 h-4 text-rose-400" />
                            <span>{language === 'en' ? 'Extraction Issues Found' : 'Problemas Encontrados'}</span>
                          </>
                        )}
                      </div>
                    </div>

                    {repairReport.saved && repairReport.saved_path && (
                      <div className="p-2.5 rounded-lg bg-slate-950/80 border border-emerald-500/25 space-y-2">
                        <div className="text-slate-400 font-semibold text-[11px]">
                          {language === 'en' ? 'Saved copy destination:' : 'Arquivo salvo em:'}
                        </div>
                        <div className="font-mono text-emerald-300 break-all select-text cursor-text bg-black/50 p-2 rounded text-[11px] border border-emerald-500/20">
                          {repairReport.saved_path}
                        </div>
                        <button
                          onClick={() => sendIpc('open_output_folder', { path: repairReport.saved_path })}
                          className="px-3 py-1.5 rounded-md bg-emerald-600 hover:bg-emerald-500 text-white font-semibold text-xs flex items-center gap-1.5 transition shadow-sm"
                        >
                          <FolderOpen className="w-3.5 h-3.5" />
                          <span>{language === 'en' ? 'Open in File Explorer' : 'Abrir Pasta no Explorador'}</span>
                        </button>
                      </div>
                    )}

                    {repairReport.fixes_applied && repairReport.fixes_applied.length > 0 && (
                      <div>
                        <div className="font-semibold text-slate-300 mb-1">
                          {language === 'en' ? 'Fixes applied:' : 'Correções aplicadas:'}
                        </div>
                        <div className="space-y-1">
                          {repairReport.fixes_applied.map((f, i) => (
                            <div key={i} className="text-slate-300 text-[11px] flex items-center gap-1.5">
                              <span className="text-emerald-400 font-bold">•</span>
                              <span>{f}</span>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {repairReport.issues_found && repairReport.issues_found.length > 0 && (
                      <div>
                        <div className="font-semibold text-slate-400 mb-1">
                          {language === 'en' ? 'Issues identified:' : 'Problemas identificados:'}
                        </div>
                        <div className="space-y-1">
                          {repairReport.issues_found.map((iss, i) => (
                            <div key={i} className="text-slate-400 text-[11px] flex items-center gap-1.5">
                              <span className="text-amber-400 font-bold">•</span>
                              <span>{iss}</span>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                )}

                {/* Metadata & Allocation */}
                <div className="border border-slate-800 rounded-lg p-3 bg-slate-950/50">
                  <div className="text-xs font-semibold text-slate-400 mb-2">
                    {language === 'en' ? 'Metadata & Allocation:' : 'Metadados & Alocação:'}
                  </div>
                  <div className="space-y-1.5 text-xs">
                    <div className="flex justify-between text-slate-400">
                      <span>{language === 'en' ? 'File Size:' : 'Tamanho do Arquivo:'}</span>
                      <span className="font-mono text-slate-200">{previewFile.size_bytes.toLocaleString()} bytes ({(previewFile.size_bytes / 1024).toFixed(1)} KB)</span>
                    </div>
                    <div className="flex justify-between text-slate-400">
                      <span>{language === 'en' ? 'Source Offset:' : 'Offset de Origem:'}</span>
                      <span className="font-mono text-slate-200">0x{previewFile.source_offset.toString(16).toUpperCase()}</span>
                    </div>
                    <div className="flex justify-between text-slate-400">
                      <span>{language === 'en' ? 'Category:' : 'Categoria:'}</span>
                      <span className="text-slate-200">{getCategoryName(previewFile.category)}</span>
                    </div>
                    <div className="flex justify-between text-slate-400">
                      <span>{language === 'en' ? 'Integrity:' : 'Integridade:'}</span>
                      <span>{getIntegrityBadge(previewFile.integrity)}</span>
                    </div>
                    {previewData?.metadata_entries && previewData.metadata_entries.map(([k, v], idx) => {
                      const translateMetaKey = (key: string) => {
                        if (language !== 'pt') return key;
                        const map: Record<string, string> = {
                          'Format': 'Formato',
                          'Dimensions': 'Dimensões',
                          'Color Type': 'Tipo de Cor',
                          'Container': 'Contêiner',
                          'MIME Type': 'Tipo MIME',
                          'Duration': 'Duração',
                          'Bitrate': 'Taxa de Bits',
                          'Sample Rate': 'Taxa de Amostragem',
                          'Channels': 'Canais',
                          'Codec': 'Codec',
                          'Page Count': 'Qtd. de Páginas',
                          'Author': 'Autor',
                          'Created': 'Criado em',
                          'Modified': 'Modificado em',
                          'Size': 'Tamanho',
                          'Type': 'Tipo',
                        };
                        return map[key] || key;
                      };
                      return (
                        <div key={idx} className="flex justify-between text-slate-400 pt-1 border-t border-slate-900">
                          <span>{translateMetaKey(k)}:</span>
                          <span className="font-mono text-slate-300 text-right truncate max-w-[240px]">{v}</span>
                        </div>
                      );
                    })}
                  </div>
                </div>
              </>
            ) : (
              /* Hex Dump Inspection Tab */
              <div className="space-y-3">
                <div className="flex items-center justify-between gap-2 p-2.5 rounded-lg border border-sky-500/20 bg-sky-500/10 text-[11px] text-sky-300">
                  <span>
                    {language === 'en'
                      ? 'Showing raw binary sector bytes (first 512 bytes) read directly from disk offsets.'
                      : 'Exibindo bytes binários brutos do setor (primeiros 512 bytes) lidos diretamente do disco.'}
                  </span>
                  {previewData?.hex_dump && previewData.hex_dump.length > 0 && (
                    <button
                      onClick={handleCopyHexDump}
                      className="shrink-0 px-2.5 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium flex items-center gap-1.5 transition border border-slate-700/50"
                      title={language === 'en' ? 'Copy hex dump to clipboard' : 'Copiar dump hexadecimal para a área de transferência'}
                    >
                      {copiedHex ? (
                        <>
                          <Check className="w-3.5 h-3.5 text-emerald-400" />
                          <span className="text-emerald-400">{language === 'en' ? 'Copied!' : 'Copiado!'}</span>
                        </>
                      ) : (
                        <>
                          <Copy className="w-3.5 h-3.5 text-slate-400" />
                          <span>{language === 'en' ? 'Copy Hex Dump' : 'Copiar Hex'}</span>
                        </>
                      )}
                    </button>
                  )}
                </div>

                {previewData?.hex_dump && previewData.hex_dump.length > 0 ? (
                  <div className="border border-slate-800 rounded-lg p-3 bg-slate-950 font-mono text-[11px] overflow-x-auto select-text cursor-text">
                    <div className="flex items-center justify-between text-slate-400 font-semibold mb-2 select-none">
                      <span>{language === 'en' ? 'Sector Hex Dump:' : 'Dump Hexadecimal do Setor:'}</span>
                      <span className="text-[10px] text-slate-500 font-normal">
                        {language === 'en' ? '(Selectable text — drag to highlight & copy)' : '(Texto selecionável — arraste para marcar e copiar)'}
                      </span>
                    </div>
                    {previewData.hex_dump.map((line, idx) => (
                      <div key={idx} className="flex gap-2 text-slate-300 hover:bg-slate-900/60 py-0.5 px-1 rounded select-text cursor-text font-mono">
                        <span className="text-slate-500 select-text cursor-text shrink-0">{line.offset}</span>
                        <span className="text-sky-300 select-text cursor-text shrink-0">{line.hex}</span>
                        <span className="text-slate-400 select-text cursor-text">|{line.ascii}|</span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-xs text-slate-500 text-center py-8">
                    {language === 'en' ? 'No hex data available for this sector.' : 'Nenhum dado hexadecimal para este setor.'}
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Settings Modal (Fully Implemented) */}
      {showSettings && (
        <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4 z-50">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl max-w-lg w-full p-6 shadow-2xl flex flex-col max-h-[90vh]">
            <div className="flex items-center justify-between pb-3 border-b border-slate-800 mb-4 shrink-0">
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-lg bg-sky-500/10 text-sky-400 flex items-center justify-center">
                  <Sliders className="w-4 h-4" />
                </div>
                <div>
                  <h3 className="font-bold text-base">{language === 'en' ? 'OpenRe Preferences & Safety' : 'Preferências & Segurança do OpenRe'}</h3>
                  <div className="text-[11px] text-slate-400">{language === 'en' ? 'Configure recovery engine behavior' : 'Configurar comportamento do motor de recuperação'}</div>
                </div>
              </div>
              <button onClick={() => setShowSettings(false)} className="text-slate-400 hover:text-slate-100 p-1">
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="flex-1 min-h-0 overflow-y-auto pr-1 space-y-5 text-xs">
              {/* Language selection */}
              <div>
                <label className="font-semibold text-slate-300 block mb-1.5">Application Language / Idioma</label>
                <div className="grid grid-cols-2 gap-2">
                  <button
                    onClick={() => setLanguage('en')}
                    className={`py-2 px-3 rounded-lg border text-left font-medium transition ${
                      language === 'en' ? 'border-sky-500 bg-sky-500/15 text-white' : 'border-slate-800 bg-slate-950 text-slate-400'
                    }`}
                  >
                    English (Default)
                  </button>
                  <button
                    onClick={() => setLanguage('pt')}
                    className={`py-2 px-3 rounded-lg border text-left font-medium transition ${
                      language === 'pt' ? 'border-sky-500 bg-sky-500/15 text-white' : 'border-slate-800 bg-slate-950 text-slate-400'
                    }`}
                  >
                    Português (Brasil)
                  </button>
                </div>
              </div>

              {/* Stride selection */}
              <div>
                <label className="font-semibold text-slate-300 block mb-1">{language === 'en' ? 'Deep Carving Stride' : 'Passo da Escavação Profunda (Stride)'}</label>
                <p className="text-[11px] text-slate-400 mb-2">{language === 'en' ? 'Block boundary alignment when searching raw disk sectors.' : 'Alinhamento dos limites de blocos ao escanear setores brutos do disco.'}</p>
                <div className="grid grid-cols-2 gap-2">
                  <button
                    onClick={() => setScanStride(512)}
                    className={`p-2.5 rounded-lg border text-left transition ${
                      scanStride === 512 ? 'border-sky-500 bg-sky-500/15 text-white' : 'border-slate-800 bg-slate-950 text-slate-400'
                    }`}
                  >
                    <div className="font-semibold text-xs">{language === 'en' ? '512 Bytes (Recommended)' : '512 Bytes (Recomendado)'}</div>
                    <div className="text-[10px] text-slate-400">{language === 'en' ? 'Maximum forensic sensitivity' : 'Sensibilidade forense máxima'}</div>
                  </button>
                  <button
                    onClick={() => setScanStride(4096)}
                    className={`p-2.5 rounded-lg border text-left transition ${
                      scanStride === 4096 ? 'border-sky-500 bg-sky-500/15 text-white' : 'border-slate-800 bg-slate-950 text-slate-400'
                    }`}
                  >
                    <div className="font-semibold text-xs">{language === 'en' ? '4096 Bytes (Fast)' : '4096 Bytes (Rápido)'}</div>
                    <div className="text-[10px] text-slate-400">{language === 'en' ? '4K cluster alignment' : 'Alinhamento de cluster 4K'}</div>
                  </button>
                </div>
              </div>

              {/* Safety Toggles */}
              <div className="space-y-3 pt-2 border-t border-slate-800">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="font-semibold text-slate-300">{language === 'en' ? 'Prevent Same-Device Recovery' : 'Bloquear Recuperação na Mesma Unidade'}</div>
                    <div className="text-[11px] text-slate-400">{language === 'en' ? 'Blocks saving files onto the scanned drive to avoid overwriting.' : 'Impede salvar arquivos no mesmo disco varrido para evitar sobrescrita.'}</div>
                  </div>
                  <input
                    type="checkbox"
                    checked={enforceSeparation}
                    onChange={(e) => setEnforceSeparation(e.target.checked)}
                    className="w-4 h-4 accent-sky-500 rounded"
                  />
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <div className="font-semibold text-slate-300">{language === 'en' ? 'Auto-Detect Truncated JPEG/PNG' : 'Auto-detectar JPEG/PNG Truncados'}</div>
                    <div className="text-[11px] text-slate-400">{language === 'en' ? 'Enables one-click repair copy button for missing EOI/CRC markers.' : 'Habilita botão de cópia com reparo para marcadores EOI/CRC ausentes.'}</div>
                  </div>
                  <input
                    type="checkbox"
                    checked={autoRepairMarkers}
                    onChange={(e) => setAutoRepairMarkers(e.target.checked)}
                    className="w-4 h-4 accent-sky-500 rounded"
                  />
                </div>
              </div>

              {/* Buffer Size */}
              <div className="pt-2 border-t border-slate-800">
                <div className="flex justify-between items-center mb-1">
                  <label className="font-semibold text-slate-300">{language === 'en' ? 'Sliding Window Buffer Size' : 'Tamanho do Buffer da Janela Deslizante'}</label>
                  <span className="font-mono text-sky-400 font-bold">{maxBufferSizeMb} MB</span>
                </div>
                <input
                  type="range"
                  min="64"
                  max="512"
                  step="64"
                  value={maxBufferSizeMb}
                  onChange={(e) => setMaxBufferSizeMb(Number(e.target.value))}
                  className="w-full accent-sky-500"
                />
              </div>

              {/* Terminal Visibility Toggle */}
              <div className="pt-2 border-t border-slate-800">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="font-semibold text-slate-300">
                      {language === 'en' ? 'Show Diagnostic Terminal in Footer' : 'Exibir Terminal de Diagnóstico no Rodapé'}
                    </div>
                    <div className="text-[11px] text-slate-400">
                      {language === 'en'
                        ? 'Displays real-time technical operations, block reads, and warnings in the bottom panel.'
                        : 'Exibe operações técnicas em tempo real, leitura de blocos e alertas no painel inferior.'}
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={showTerminal}
                    onChange={(e) => setShowTerminal(e.target.checked)}
                    className="w-4 h-4 accent-sky-500 rounded cursor-pointer"
                  />
                </div>
              </div>
            </div>

            <div className="shrink-0 pt-4 border-t border-slate-800 flex justify-end gap-2 mt-4">
              <button
                onClick={() => setShowSettings(false)}
                className="px-5 py-2 rounded-lg bg-sky-600 hover:bg-sky-500 text-xs font-semibold text-white shadow-md transition"
              >
                {language === 'en' ? 'Save & Close' : 'Salvar & Fechar'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* About Modal */}
      {showAbout && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 z-50">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl max-w-md w-full p-6 shadow-2xl">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-lg bg-sky-600 flex items-center justify-center font-bold text-white">Re</div>
                <h3 className="font-bold text-base">{language === 'en' ? 'About OpenRe' : 'Sobre o OpenRe'}</h3>
              </div>
              <button onClick={() => setShowAbout(false)} className="text-slate-400 hover:text-slate-100">
                <X className="w-5 h-5" />
              </button>
            </div>
            <p className="text-xs text-slate-400 mb-4 leading-relaxed">
              {language === 'en'
                ? 'OpenRe is an open-source, privacy-first desktop application for deleted file recovery, disk carving, and forensic file inspection.'
                : 'O OpenRe é um aplicativo desktop de código aberto focado em privacidade para recuperação de arquivos deletados, escavação de disco e inspeção forense.'}
            </p>
            <div className="text-xs space-y-2 border-t border-slate-800 pt-4 text-slate-300">
              <div><strong>{language === 'en' ? 'License:' : 'Licença:'}</strong> MIT Open Source</div>
              <div><strong>{language === 'en' ? 'Core Engine:' : 'Motor Principal:'}</strong> Rust 2021 (Memory-Safe)</div>
              <div><strong>{language === 'en' ? 'Privacy:' : 'Privacidade:'}</strong> {language === 'en' ? '100% Offline (Zero telemetry)' : '100% Offline (Zero telemetria)'}</div>
              <div><strong>{language === 'en' ? 'Portuguese Guide:' : 'Guia em Português:'}</strong> docs/installation/INSTALLATION_PT_BR.md</div>
            </div>
            <button
              onClick={() => setShowAbout(false)}
              className="mt-6 w-full py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 transition"
            >
              {language === 'en' ? 'Close' : 'Fechar'}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
