export type TrackStatus =
  | "pending"
  | "resolving"
  | "ready"
  | "no_audio"
  | "failed";

export interface Forum {
  name: string;
  url: string;
  parent_url: string | null;
  children: Forum[];
}

export interface Track {
  id: string;
  title: string;
  artist: string;
  author: string;
  uploaded_at: string;
  last_post_at: string;
  replies: number;
  views: number;
  thanks_count: number;
  thumb_url: string | null;
  forum: string;
  forum_url: string;
  post_url: string;
  beatexs_ids: string[];
  beatexs_url: string | null;
  reaction_url: string | null;
  audio_url: string | null;
  art_url: string | null;
  duration_secs: number | null;
  peaks: number[];
  thanked: boolean;
  hidden: boolean;
  status: TrackStatus;
  updated_at: number;
}

export interface QualityVariant {
  id: string;
  filename: string;
  format: string;
  quality_label: string;
  size_bytes: number;
  lossless: boolean;
  page_url: string;
  score: number;
}

export interface Settings {
  theme: string;
  volume: number;
  min_delay_ms: number;
  max_per_host_per_hour: number;
  max_per_session: number;
  metadata_ttl_days: number;
  cache_limit_mb: number;
  prefetch_next: boolean;
  media_keys: boolean;
  download_dir: string;
  auto_like_on_download: boolean;
}

export type RepeatMode = "off" | "all" | "one";

export interface UiPreferences {
  show_hidden: boolean;
  row_scale: number;
  artist_first: boolean;
  sort_by: string;
  descending: boolean;
  columns: string[];
  column_widths: Record<string, number>;
  favorites: string[];
  collapsed: string[];
}

export interface FrontendState {
  tracks: Track[];
  current_index: number;
  playing: boolean;
  position: number;
  duration: number;
  volume: number;
  signed_in: boolean;
  shuffle: boolean;
  repeat: RepeatMode;
  settings: Settings;
  prefs: UiPreferences;
  current_forum: Forum | null;
  now_playing: Track | null;
  loaded_pages: number[];
  total_pages: number;
  liked_count: number;
  portable: boolean;
  needs_data_dir: boolean;
  data_dir: string;
}

export interface TrackBatch {
  tracks: Track[];
  loaded_pages: number[];
  total_pages: number;
  errors: string[];
}

export interface Tick {
  position: number;
  duration: number;
  playing: boolean;
  index: number;
}

export interface LoadProgress {
  done: number;
  total: number;
  title: string;
}

export interface DownloadProgress {
  id: string;
  done: number;
  total: number;
}

export type DownloadOutcome =
  | { status: "needs_choice"; value: QualityVariant[] }
  | { status: "needs_sign_in" }
  | { status: "complete"; value: { path: string; warning: string | null } };
