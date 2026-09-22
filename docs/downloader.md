# Downloader

Each file is anchored to official HTTPS metadata, an approved Mojang/Minecraft hostname, an expected SHA-1 (or supported SHA-256), and an expected size when supplied. TLS verification stays enabled; redirects are disabled.

Files download with bounded concurrency (1–16), three attempts, unique partial filenames in the destination directory, incremental transfer events, verification, and same-directory rename. Corrupt cached files are replaced only after a verified replacement has been downloaded. Cancellation is checked between chunks and before launch. Verified cache entries are reused.

Progress reports completed files, total files in the active stage, actual transferred bytes, phase, and throughput. Stages include version JSON, Java runtime files, asset index, and game files; progress resets at stage boundaries. No simulated progress.

No Minecraft binaries are committed or distributed with NodeClient. Source tests use tiny generated fixtures.
