export type ProviderType = "xtream" | "m3u";

// Timestamps cross the Tauri IPC boundary as Unix seconds.
export interface Provider {
  id: string;
  name: string;
  type: ProviderType;
  serverUrl: string | null;
  username: string | null;
  playlistUrl: string | null;
  localFilePath: string | null;
  lastRefreshed: number | null;
  createdAt: number;
}

export interface ProviderInput {
  id?: string;
  name: string;
  type: ProviderType;
  serverUrl?: string;
  username?: string;
  password?: string;
  playlistUrl?: string;
  localFilePath?: string;
}

export interface XtreamAccountInfo {
  status: string | null;
  expDate: number | null;
  maxConnections: number | null;
  activeConnections: number | null;
}

export interface ConnectionTestResult {
  success: boolean;
  message: string;
  accountInfo: XtreamAccountInfo | null;
}

export interface Category {
  id: string;
  name: string;
  sortOrder: number;
}

export interface LiveChannel {
  id: string;
  name: string;
  categoryId: string;
  categoryName: string;
  logoUrl: string | null;
  streamUrl: string;
  streamExt: string;
  epgChannelId: string | null;
}

export interface PaginatedResult<T> {
  items: T[];
  total: number;
  page: number; // 1-based
  pageSize: number;
}

export interface CatalogSummary {
  liveChannels: number;
  movies: number;
  series: number;
}

export interface RefreshProgress {
  stage: string;
  progress: number; // 0..1
}

export interface RefreshComplete {
  success: boolean;
  error?: string;
}
