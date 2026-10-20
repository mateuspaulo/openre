import React, { useState, useEffect } from 'react';
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

  // Terminal & Diagnostics state
  const [showTerminal, setShowTerminal] = useState<boolean>(true);
  const [terminalLogs, setTerminalLogs] = useState<LogEntry[]>([
    {
      id: 'init-1',
      timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
      level: 'info',
      message: 'OpenRe v0.1.2 engine initialized in strict offline-first mode. Zero telemetry active.',
    },
    {
      id: 'init-2',
      timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
      level: 'success',
      message: 'All device & partition readers configured for strictly non-destructive read-only operation.',
    },
  ]);
  const terminalEndRef = React.useRef<HTMLDivElement | null>(null);

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
          resolve(event.detail.data);
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
      const { type, payload } = e.detail || {};
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
        setDiscoveredFiles(payload.files || []);
        setSelectedFileIds(new Set((payload.files || []).map((f: any) => f.id)));
        setCurrentStep(5);
        appendLog('success', `Scan complete. Found ${(payload.files || []).length} recoverable items.`);
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
    setIsScanning(true);
    setScanError(null);
    setCurrentStep(4);
    appendLog('info', `Starting ${scanMode.toUpperCase()} scan on ${selectedSource.name}...`);
    setScanProgress({
      percent: 0,
      scannedBytes: 0,
      totalBytes: selectedSource.size_bytes || 1024 * 1024 * 1024,
      speedMBps: 0,
      filesCount: 0,
      currentLabel: 'Starting scan...',
    });

    sendIpc('start_scan', {
      source_id: selectedSource.id,
      mode: scanMode,
      categories: selectedCategories,
    });
  };

  const handleCancelScan = () => {
    appendLog('warn', 'Scan cancel requested by user.');
    sendIpc('cancel_scan');
    setIsScanning(false);
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
            `Destination drive (${destLetter}) matches the source drive being recovered! Writing files to the same drive overwrites deleted data. Select a different drive.`
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
    setRecoveryReportMd(res?.report_markdown || 'Recovery report generated successfully.');
    setCurrentStep(7);
  };

  const openPreview = async (file: RecoveredFile) => {
    setPreviewFile(file);
    setRepairReport(null);
    setPreviewTab('media');
    const data = await sendIpc('get_file_preview', {
      source_id: selectedSource?.id,
      source_offset: file.source_offset,
      size_bytes: file.size_bytes,
      extension: file.extension,
    });
    setPreviewData(data);
  };

  const handleRepairCopy = async () => {
    if (!previewFile) return;
    const report = await sendIpc('repair_file', {
      source_id: selectedSource?.id,
      source_offset: previewFile.source_offset,
      size_bytes: previewFile.size_bytes,
      filename: previewFile.filename,
    });
    setRepairReport(report);
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

  const getIntegrityBadge = (status: string) => {
    if (status.includes('Likely')) {
      return <span className="px-2 py-0.5 text-xs rounded-full bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">Likely</span>;
    }
    if (status.includes('Partially')) {
      return <span className="px-2 py-0.5 text-xs rounded-full bg-amber-500/20 text-amber-400 border border-amber-500/30">Partial</span>;
    }
    return <span className="px-2 py-0.5 text-xs rounded-full bg-rose-500/20 text-rose-400 border border-rose-500/30">Corrupted</span>;
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
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-sky-500/20 text-sky-400 border border-sky-500/30">v0.1.2</span>
            </div>
            <div className="text-[11px] text-slate-400">Professional File Recovery</div>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 text-xs">
            <ShieldCheck className="w-3.5 h-3.5" />
            <span>100% Offline & Private</span>
          </div>

          {/* Diagnostic Terminal Toggle Button */}
          <button
            onClick={() => setShowTerminal(!showTerminal)}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg border text-xs font-mono transition ${
              showTerminal
                ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-400'
                : 'bg-slate-800/80 border-slate-700 text-slate-400 hover:text-slate-200'
            }`}
            title={showTerminal ? 'Hide Terminal (Rodapé)' : 'Show Terminal (Rodapé)'}
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
            title="Toggle theme"
          >
            {theme === 'dark' ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
          </button>

          <button
            onClick={() => setShowSettings(true)}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-100 hover:bg-slate-800 transition relative"
            title="Open Settings"
          >
            <Settings className="w-4 h-4" />
          </button>

          <button
            onClick={() => setShowAbout(true)}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-100 hover:bg-slate-800 transition"
            title="About OpenRe"
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
              {['Source', 'Method', 'Filters', 'Scan', 'Review', 'Recover', 'Report'].map((label, idx) => {
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
                Cancel to Dashboard
              </button>
            )}
          </div>
        )}

        {/* STEP 0: Initial Screen / Dashboard */}
        {currentStep === 0 && (
          <div className="flex-1 min-h-0 overflow-y-auto flex flex-col items-center justify-center max-w-4xl mx-auto w-full">
            <div className="text-center mb-8">
              <h1 className="text-3xl font-bold tracking-tight mb-2">Recover Your Lost Files Safely</h1>
              <p className="text-slate-400 max-w-xl mx-auto text-sm">
                OpenRe performs high-performance deep sector carving and filesystem undeletion with guaranteed offline privacy.
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
                <h3 className="font-semibold text-base mb-1">Recover Files</h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  Step-by-step guided recovery wizard for deleted files, pictures, and documents.
                </p>
                <div className="text-xs font-medium text-sky-400 flex items-center gap-1">
                  Start Wizard <ArrowRight className="w-3.5 h-3.5" />
                </div>
              </div>

              <div
                onClick={() => { setScanMode('deep'); setCurrentStep(1); }}
                className="p-5 rounded-xl border border-slate-800 bg-slate-900/50 hover:border-indigo-500/50 hover:bg-slate-800/50 cursor-pointer transition flex flex-col group shadow-sm"
              >
                <div className="w-10 h-10 rounded-lg bg-indigo-500/10 text-indigo-400 flex items-center justify-center mb-4 group-hover:scale-110 transition">
                  <HardDrive className="w-5 h-5" />
                </div>
                <h3 className="font-semibold text-base mb-1">Scan a Drive</h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  Deep block-strided signature carving across damaged, unpartitioned, or formatted disks.
                </p>
                <div className="text-xs font-medium text-indigo-400 flex items-center gap-1">
                  Select Drive <ArrowRight className="w-3.5 h-3.5" />
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
                <h3 className="font-semibold text-base mb-1">Scan a Disk Image</h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  Load raw image dumps (.img, .dd, .raw) safely without accessing the physical media repeatedly.
                </p>
                <div className="text-xs font-medium text-emerald-400 flex items-center gap-1">
                  Browse Image <ArrowRight className="w-3.5 h-3.5" />
                </div>
              </div>
            </div>

            <div className="text-xs text-slate-500 flex items-center gap-6">
              <span>✓ NTFS / FAT32 / exFAT / ext4</span>
              <span>✓ 15+ Format-Aware Carvers</span>
              <span>✓ Pre-Recovery Hex/Image Sandbox Preview</span>
            </div>
          </div>
        )}

        {/* STEP 1: Select Source (Fixed scrolling & visible footer) */}
        {currentStep === 1 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-4xl mx-auto w-full">
            <div className="shrink-0 mb-3 flex items-center justify-between">
              <div>
                <h2 className="text-xl font-bold">Step 1 — Select Storage Media</h2>
                <p className="text-xs text-slate-400">
                  Select a drive below, or double-click to proceed directly.
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
                  <FolderOpen className="w-3.5 h-3.5 text-sky-400" /> Browse Folder...
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
                  <Database className="w-3.5 h-3.5 text-emerald-400" /> Browse Image (.img)...
                </button>
              </div>
            </div>

            {/* Scrollable Device List Container with Min-H-0 */}
            <div className="flex-1 min-h-0 overflow-y-auto pr-3 space-y-2 border border-slate-800/80 rounded-xl p-3 bg-slate-950/40">
              <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-400 mb-1 flex items-center justify-between">
                <span>Logical Partitions & Volumes</span>
                <span className="text-slate-500 text-[10px]">Click to select, double-click to advance</span>
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
                      {dev.size_bytes > 0 ? `${(dev.size_bytes / (1024 * 1024 * 1024)).toFixed(1)} GB` : 'Available'}
                    </div>
                  </div>
                );
              })}

              <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-400 mt-5 mb-1">Physical Hard Drives & SSDs</div>
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
                      Admin Elevation
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
                Back to Dashboard
              </button>

              <div className="flex items-center gap-4">
                <span className="text-xs text-slate-400">
                  {selectedSource ? (
                    <span className="text-slate-200 font-medium">Selected: <strong>{selectedSource.name}</strong></span>
                  ) : (
                    'Select a drive to proceed'
                  )}
                </span>
                <button
                  disabled={!selectedSource}
                  onClick={() => setCurrentStep(2)}
                  className="px-6 py-2.5 rounded-lg bg-sky-600 hover:bg-sky-500 disabled:opacity-40 text-xs font-semibold text-white shadow-md shadow-sky-600/25 transition flex items-center gap-2"
                >
                  Continue to Recovery Method <ArrowRight className="w-4 h-4" />
                </button>
              </div>
            </div>
          </div>
        )}

        {/* STEP 2: Select Method */}
        {currentStep === 2 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-4xl mx-auto w-full">
            <h2 className="text-xl font-bold mb-1">Step 2 — Select Recovery Method</h2>
            <p className="text-xs text-slate-400 mb-6">Choose how OpenRe should analyze the selected media.</p>

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
                <h3 className="font-semibold text-sm mb-1">Quick Scan (Filesystem Undelete)</h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  Fastest option. Inspects NTFS ($MFT), FAT32, exFAT, and ext4 directory metadata to recover recently deleted files with original filenames, timestamps, and directories intact.
                </p>
                <div className="text-xs text-emerald-400 font-medium">✓ Preserves original file names & paths</div>
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
                <h3 className="font-semibold text-sm mb-1">Deep Scan (Signature Carving)</h3>
                <p className="text-xs text-slate-400 mb-4 flex-1">
                  Examines raw sector blocks across the entire device. Detects file headers and chunk boundaries (JPEG, PNG, PDF, DOCX, ZIP, MP4) even on formatted or corrupted drives.
                </p>
                <div className="text-xs text-indigo-400 font-medium">✓ Works on formatted or RAW partitions</div>
              </div>
            </div>

            <div className="shrink-0 pt-4 border-t border-slate-800 flex items-center justify-between mt-auto">
              <button
                onClick={() => setCurrentStep(1)}
                className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800 transition"
              >
                Back
              </button>
              <button
                onClick={() => setCurrentStep(3)}
                className="px-5 py-2.5 rounded-lg bg-sky-600 hover:bg-sky-500 text-xs font-semibold text-white shadow-md shadow-sky-600/25 transition flex items-center gap-1.5"
              >
                Continue to Filters <ArrowRight className="w-4 h-4" />
              </button>
            </div>
          </div>
        )}

        {/* STEP 3: Apply Filters */}
        {currentStep === 3 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-4xl mx-auto w-full">
            <h2 className="text-xl font-bold mb-1">Step 3 — Apply Search Filters</h2>
            <p className="text-xs text-slate-400 mb-6">Select which file types you are looking to recover.</p>

            <div className="grid grid-cols-3 gap-3 mb-8">
              {[
                { label: 'Images', ext: 'JPG, PNG, GIF, WebP, BMP', key: 'Image', icon: ImageIcon },
                { label: 'Documents', ext: 'PDF, DOCX, XLSX, PPTX', key: 'Document', icon: FileText },
                { label: 'Videos', ext: 'MP4, MKV, MOV, AVI', key: 'Video', icon: Video },
                { label: 'Audio', ext: 'MP3, WAV, FLAC', key: 'Audio', icon: Music },
                { label: 'Archives', ext: 'ZIP, RAR, 7Z, TAR', key: 'Archive', icon: Archive },
                { label: 'Code & Text', ext: 'TXT, JSON, MD, LOG', key: 'Text', icon: FileSearch },
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
                Back
              </button>
              <button
                onClick={handleStartScan}
                className="px-6 py-2.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-xs font-semibold text-white shadow-lg shadow-emerald-600/25 transition flex items-center gap-2"
              >
                <Play className="w-4 h-4 fill-current" /> Start Scan Now
              </button>
            </div>
          </div>
        )}

        {/* STEP 4: Scanning Screen */}
        {currentStep === 4 && (
          <div className="flex-1 min-h-0 flex flex-col items-center justify-center max-w-2xl mx-auto w-full p-4">
            {scanError ? (
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
                  <span>Speed: {scanProgress.speedMBps} MB/s</span>
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
          <div className="flex-1 min-h-0 flex flex-col h-full overflow-hidden">
            <div className="shrink-0 flex items-center justify-between gap-4 mb-3 pb-3 border-b border-slate-800">
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  placeholder={language === 'en' ? "Filter by name or extension..." : "Filtrar por nome ou extensão..."}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="px-3 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-sky-500 w-60"
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

                <div className="flex items-center gap-1.5 ml-2">
                  <button
                    onClick={toggleSelectPage}
                    className="px-2 py-1 rounded border border-slate-800 bg-slate-900 hover:bg-slate-800 text-[11px] text-slate-300 transition"
                  >
                    {isPageFullySelected
                      ? (language === 'en' ? 'Deselect Page' : 'Desmarcar Página')
                      : (language === 'en' ? 'Select Page' : 'Marcar Página')}
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

              <div className="flex items-center gap-3">
                <div className="text-xs text-slate-400 font-mono">
                  <strong className="text-sky-400">{selectedFileIds.size}</strong> of {filteredFiles.length} selected
                </div>

                <div className="flex items-center border border-slate-800 rounded-lg overflow-hidden">
                  <button
                    onClick={() => setViewMode('list')}
                    className={`p-1.5 ${viewMode === 'list' ? 'bg-slate-800 text-sky-400' : 'text-slate-400 hover:text-slate-200'}`}
                  >
                    <List className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => setViewMode('grid')}
                    className={`p-1.5 ${viewMode === 'grid' ? 'bg-slate-800 text-sky-400' : 'text-slate-400 hover:text-slate-200'}`}
                  >
                    <LayoutGrid className="w-4 h-4" />
                  </button>
                </div>

                <button
                  onClick={() => setCurrentStep(6)}
                  disabled={selectedFileIds.size === 0}
                  className="px-4 py-1.5 rounded-lg bg-sky-600 hover:bg-sky-500 disabled:opacity-50 text-xs font-semibold text-white shadow-md shadow-sky-600/25 transition flex items-center gap-1.5"
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
                    {paginatedFiles.map((file) => (
                      <tr key={file.id} className="hover:bg-slate-900/60 transition group">
                        <td className="py-2 px-3">
                          <input
                            type="checkbox"
                            checked={selectedFileIds.has(file.id)}
                            onChange={(e) => {
                              const next = new Set(selectedFileIds);
                              if (e.target.checked) next.add(file.id);
                              else next.delete(file.id);
                              setSelectedFileIds(next);
                            }}
                          />
                        </td>
                        <td className="py-2 px-3 flex items-center gap-2 font-mono">
                          {getCategoryIcon(file.category)}
                          <span className="font-medium text-slate-200">{file.filename}</span>
                        </td>
                        <td className="py-2 px-3 text-slate-400">{file.category}</td>
                        <td className="py-2 px-3 text-slate-400 font-mono">
                          {(file.size_bytes / 1024).toFixed(1)} KB
                        </td>
                        <td className="py-2 px-3">{getIntegrityBadge(file.integrity)}</td>
                        <td className="py-2 px-3 text-right">
                          <button
                            onClick={() => openPreview(file)}
                            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs transition inline-flex items-center gap-1"
                          >
                            <Eye className="w-3.5 h-3.5" /> {language === 'en' ? 'Preview' : 'Visualizar'}
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : (
                <div className="grid grid-cols-4 gap-3 p-1">
                  {paginatedFiles.map((file) => (
                    <div
                      key={file.id}
                      onClick={() => openPreview(file)}
                      className={`p-3 rounded-lg border transition flex flex-col justify-between cursor-pointer ${
                        selectedFileIds.has(file.id)
                          ? 'border-sky-500/60 bg-sky-500/10'
                          : 'border-slate-800 bg-slate-900/40 hover:border-slate-700'
                      }`}
                    >
                      <div className="flex items-center justify-between mb-2">
                        <div className="flex items-center gap-2">
                          <input
                            type="checkbox"
                            checked={selectedFileIds.has(file.id)}
                            onClick={(e) => e.stopPropagation()}
                            onChange={(e) => {
                              const next = new Set(selectedFileIds);
                              if (e.target.checked) next.add(file.id);
                              else next.delete(file.id);
                              setSelectedFileIds(next);
                            }}
                          />
                          {getCategoryIcon(file.category)}
                        </div>
                        {getIntegrityBadge(file.integrity)}
                      </div>
                      <div className="font-mono text-xs font-medium truncate text-slate-200 mb-1">{file.filename}</div>
                      <div className="flex items-center justify-between text-[11px] text-slate-400 font-mono">
                        <span>{(file.size_bytes / 1024).toFixed(1)} KB</span>
                        <span className="text-sky-400 hover:underline">{language === 'en' ? 'Preview' : 'Ver'}</span>
                      </div>
                    </div>
                  ))}
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
        )}

        {/* STEP 6: Recover Destination */}
        {currentStep === 6 && (
          <div className="flex-1 min-h-0 flex flex-col max-w-2xl mx-auto w-full justify-center">
            <h2 className="text-2xl font-bold mb-2">Step 6 — Choose Destination Directory</h2>
            <p className="text-xs text-slate-400 mb-6">Select a safe location to write the recovered files.</p>

            <div className="p-4 rounded-xl border border-slate-800 bg-slate-900/60 mb-4">
              <label className="text-xs font-semibold text-slate-300 block mb-2">Recovery Target Directory:</label>
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  readOnly
                  placeholder="Click Browse to choose destination..."
                  value={destPath}
                  className="flex-1 px-3 py-2 rounded-lg bg-slate-950 border border-slate-800 text-xs text-slate-200 font-mono focus:outline-none"
                />
                <button
                  onClick={handleSelectDestFolder}
                  className="px-4 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200 transition flex items-center gap-1.5"
                >
                  <FolderOpen className="w-4 h-4" /> Browse
                </button>
              </div>
            </div>

            {collisionWarning ? (
              <div className="p-4 rounded-xl border border-rose-500/40 bg-rose-500/10 text-rose-300 text-xs flex items-start gap-3 mb-6">
                <AlertTriangle className="w-5 h-5 text-rose-400 shrink-0 mt-0.5" />
                <div>
                  <div className="font-semibold mb-1">Overwriting Risk Detected!</div>
                  <div>{collisionWarning}</div>
                </div>
              </div>
            ) : (
              <div className="p-3.5 rounded-lg border border-slate-800 bg-slate-900/40 text-slate-400 text-xs flex items-center gap-2 mb-6">
                <ShieldCheck className="w-4 h-4 text-emerald-400" />
                <span>Files will be written with SHA-256 validation. Existing files are never overwritten.</span>
              </div>
            )}

            <div className="shrink-0 pt-4 border-t border-slate-800 flex items-center justify-between">
              <button
                onClick={() => setCurrentStep(5)}
                className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800 transition"
              >
                Back to Results
              </button>
              <button
                disabled={!destPath || (!!collisionWarning && enforceSeparation) || isRecovering}
                onClick={handleStartRecovery}
                className="px-6 py-2.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-xs font-semibold text-white shadow-lg shadow-emerald-600/25 transition flex items-center gap-2"
              >
                <Download className="w-4 h-4" /> Start Recovery Process
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
              <h2 className="text-2xl font-bold">Recovery Completed Successfully</h2>
              <p className="text-xs text-slate-400">All chosen files have been written to the destination directory.</p>
            </div>

            <div className="flex-1 min-h-0 bg-slate-900 rounded-xl border border-slate-800 p-4 font-mono text-xs overflow-y-auto mb-6 whitespace-pre-wrap text-slate-300">
              {recoveryReportMd}
            </div>

            <div className="shrink-0 flex items-center justify-between pt-4 border-t border-slate-800">
              <button
                onClick={() => setCurrentStep(0)}
                className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800 transition flex items-center gap-1.5"
              >
                <RotateCcw className="w-3.5 h-3.5" /> Start New Recovery
              </button>

              <button
                onClick={() => sendIpc('open_output_folder', { path: destPath })}
                className="px-5 py-2 rounded-lg bg-sky-600 hover:bg-sky-500 text-xs font-semibold text-white shadow-md shadow-sky-600/25 transition flex items-center gap-1.5"
              >
                <FolderOpen className="w-4 h-4" /> Open Destination Folder
              </button>
            </div>
          </div>
        )}
      </main>

      {/* Docked Footer Diagnostic Terminal */}
      {showTerminal && (
        <div className="shrink-0 border-t border-slate-800 bg-slate-950 flex flex-col transition-all duration-150 shadow-2xl z-30 select-none">
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
          <div className="h-32 max-h-40 overflow-y-auto p-2.5 font-mono text-[11px] space-y-1 select-text bg-black/80">
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
      {previewFile && (
        <div className="fixed inset-y-0 right-0 w-[540px] bg-slate-900 border-l border-slate-800 shadow-2xl flex flex-col z-50">
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
                    {previewData?.media_data_url || previewData?.thumbnail_base64 ? (
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
                      {previewFile.extension.toUpperCase()} Audio • {(previewFile.size_bytes / (1024 * 1024)).toFixed(2)} MB
                    </div>
                    {previewData?.media_data_url ? (
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
                    {previewData?.media_data_url ? (
                      <div className="w-full flex flex-col items-center">
                        <video
                          controls
                          className="w-full max-h-64 rounded-lg bg-black border border-slate-800 object-contain shadow-lg"
                          src={previewData.media_data_url}
                          preload="metadata"
                        />
                        <div className="w-full flex justify-between items-center text-[10px] text-slate-400 mt-2 font-mono px-1">
                          <span className="text-purple-400 font-semibold">{previewFile.extension.toUpperCase()} Video</span>
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

                {/* Image Repair Option */}
                {(previewFile.extension === 'jpg' || previewFile.extension === 'png') && (
                  <div className="p-3 rounded-lg border border-indigo-500/30 bg-indigo-500/10 flex items-center justify-between">
                    <div>
                      <div className="font-semibold text-xs text-indigo-300">Non-Destructive Image Repair</div>
                      <div className="text-[11px] text-slate-400">Reconstruct missing markers and recalculate chunk CRCs.</div>
                    </div>
                    <button
                      onClick={handleRepairCopy}
                      className="px-3 py-1.5 rounded-md bg-indigo-600 hover:bg-indigo-500 text-xs font-semibold text-white transition flex items-center gap-1"
                    >
                      <Wrench className="w-3.5 h-3.5" /> Repair Copy
                    </button>
                  </div>
                )}

                {repairReport && (
                  <div className="p-3 rounded-lg border border-emerald-500/30 bg-emerald-500/10 text-xs text-emerald-300">
                    <div className="font-semibold mb-1">Repair Applied:</div>
                    {repairReport.fixes_applied.map((f, i) => (
                      <div key={i}>• {f}</div>
                    ))}
                  </div>
                )}

                {/* Metadata & Allocation */}
                <div className="border border-slate-800 rounded-lg p-3 bg-slate-950/50">
                  <div className="text-xs font-semibold text-slate-400 mb-2">Metadata & Allocation:</div>
                  <div className="space-y-1.5 text-xs">
                    <div className="flex justify-between text-slate-400">
                      <span>File Size:</span>
                      <span className="font-mono text-slate-200">{previewFile.size_bytes.toLocaleString()} bytes ({(previewFile.size_bytes / 1024).toFixed(1)} KB)</span>
                    </div>
                    <div className="flex justify-between text-slate-400">
                      <span>Source Offset:</span>
                      <span className="font-mono text-slate-200">0x{previewFile.source_offset.toString(16).toUpperCase()}</span>
                    </div>
                    <div className="flex justify-between text-slate-400">
                      <span>Category:</span>
                      <span className="text-slate-200">{previewFile.category}</span>
                    </div>
                    <div className="flex justify-between text-slate-400">
                      <span>Integrity:</span>
                      <span>{getIntegrityBadge(previewFile.integrity)}</span>
                    </div>
                    {previewData?.metadata_entries && previewData.metadata_entries.map(([k, v], idx) => (
                      <div key={idx} className="flex justify-between text-slate-400 pt-1 border-t border-slate-900">
                        <span>{k}:</span>
                        <span className="font-mono text-slate-300 text-right truncate max-w-[240px]">{v}</span>
                      </div>
                    ))}
                  </div>
                </div>
              </>
            ) : (
              /* Hex Dump Inspection Tab */
              <div className="space-y-3">
                <div className="p-2.5 rounded-lg border border-sky-500/20 bg-sky-500/10 text-[11px] text-sky-300">
                  {language === 'en'
                    ? 'Showing raw binary sector bytes (first 512 bytes) read directly from disk offsets.'
                    : 'Exibindo bytes binários brutos do setor (primeiros 512 bytes) lidos diretamente do disco.'}
                </div>

                {previewData?.hex_dump && previewData.hex_dump.length > 0 ? (
                  <div className="border border-slate-800 rounded-lg p-3 bg-slate-950 font-mono text-[11px] overflow-x-auto">
                    <div className="text-slate-400 font-semibold mb-2">Sector Hex Dump:</div>
                    {previewData.hex_dump.map((line, idx) => (
                      <div key={idx} className="flex gap-2 text-slate-300">
                        <span className="text-slate-500">{line.offset}</span>
                        <span className="text-sky-300">{line.hex}</span>
                        <span className="text-slate-400">|{line.ascii}|</span>
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
                  <h3 className="font-bold text-base">OpenRe Preferences & Safety</h3>
                  <div className="text-[11px] text-slate-400">Configure recovery engine behavior</div>
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
                <label className="font-semibold text-slate-300 block mb-1">Deep Carving Stride</label>
                <p className="text-[11px] text-slate-400 mb-2">Block boundary alignment when searching raw disk sectors.</p>
                <div className="grid grid-cols-2 gap-2">
                  <button
                    onClick={() => setScanStride(512)}
                    className={`p-2.5 rounded-lg border text-left transition ${
                      scanStride === 512 ? 'border-sky-500 bg-sky-500/15 text-white' : 'border-slate-800 bg-slate-950 text-slate-400'
                    }`}
                  >
                    <div className="font-semibold text-xs">512 Bytes (Recommended)</div>
                    <div className="text-[10px] text-slate-400">Maximum forensic sensitivity</div>
                  </button>
                  <button
                    onClick={() => setScanStride(4096)}
                    className={`p-2.5 rounded-lg border text-left transition ${
                      scanStride === 4096 ? 'border-sky-500 bg-sky-500/15 text-white' : 'border-slate-800 bg-slate-950 text-slate-400'
                    }`}
                  >
                    <div className="font-semibold text-xs">4096 Bytes (Fast)</div>
                    <div className="text-[10px] text-slate-400">4K cluster alignment</div>
                  </button>
                </div>
              </div>

              {/* Safety Toggles */}
              <div className="space-y-3 pt-2 border-t border-slate-800">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="font-semibold text-slate-300">Prevent Same-Device Recovery</div>
                    <div className="text-[11px] text-slate-400">Blocks saving files onto the scanned drive to avoid overwriting.</div>
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
                    <div className="font-semibold text-slate-300">Auto-Detect Truncated JPEG/PNG</div>
                    <div className="text-[11px] text-slate-400">Enables one-click repair copy button for missing EOI/CRC markers.</div>
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
                  <label className="font-semibold text-slate-300">Sliding Window Buffer Size</label>
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
                Save & Close
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
                <h3 className="font-bold text-base">About OpenRe</h3>
              </div>
              <button onClick={() => setShowAbout(false)} className="text-slate-400 hover:text-slate-100">
                <X className="w-5 h-5" />
              </button>
            </div>
            <p className="text-xs text-slate-400 mb-4 leading-relaxed">
              OpenRe is an open-source, privacy-first desktop application for deleted file recovery, disk carving, and forensic file inspection.
            </p>
            <div className="text-xs space-y-2 border-t border-slate-800 pt-4 text-slate-300">
              <div><strong>License:</strong> MIT Open Source</div>
              <div><strong>Core Engine:</strong> Rust 2021 (Memory-Safe)</div>
              <div><strong>Privacy:</strong> 100% Offline (Zero telemetry)</div>
              <div><strong>Portuguese Guide:</strong> docs/installation/INSTALLATION_PT_BR.md</div>
            </div>
            <button
              onClick={() => setShowAbout(false)}
              className="mt-6 w-full py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 transition"
            >
              Close
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
