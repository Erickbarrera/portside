interface ImportMetaEnv {
  /** Build only: raises the GitHub API rate limit for the download button's release lookup. */
  readonly GITHUB_TOKEN?: string;
  readonly PUBLIC_PORTSIDE_CATALOG_URL?: string;
  readonly PORTSIDE_CATALOG_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
