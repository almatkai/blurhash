import {
  Detail,
  ActionPanel,
  Action,
  showToast,
  Toast,
  Clipboard,
  getSelectedFinderItems,
  List,
  Icon,
  open,
} from "@raycast/api";
import { useEffect, useState } from "react";
import { execFileSync } from "node:child_process";
import { encode, decode } from "blurhash";
import path from "path";
import fs from "fs";
import os from "os";

const VIDEO_EXTS = new Set([
  ".mp4",
  ".mov",
  ".avi",
  ".mkv",
  ".webm",
  ".m4v",
  ".flv",
  ".wmv",
  ".3gp",
  ".ts",
]);
const IMAGE_EXTS = new Set([
  ".jpg",
  ".jpeg",
  ".png",
  ".gif",
  ".bmp",
  ".webp",
  ".tiff",
  ".tif",
  ".heic",
  ".heif",
  ".avif",
  ".ico",
]);
const SUPPORTED_EXTS = new Set([...VIDEO_EXTS, ...IMAGE_EXTS]);

function gcd(a: number, b: number): number {
  return b === 0 ? a : gcd(b, a % b);
}

function aspectRatio(w: number, h: number): string {
  const d = gcd(w, h);
  return `${w / d}:${h / d}`;
}

function findBinary(name: string): string | null {
  const executable = process.platform === "win32" ? `${name}.exe` : name;
  const knownPaths =
    process.platform === "darwin"
      ? [`/opt/homebrew/bin/${name}`, `/usr/local/bin/${name}`, `/usr/bin/${name}`]
      : [];
  for (const p of knownPaths) {
    if (fs.existsSync(p)) return p;
  }
  try {
    const locator = process.platform === "win32" ? "where.exe" : "which";
    return execFileSync(locator, [executable], { encoding: "utf-8", windowsHide: true })
      .split(/\r?\n/)
      .find(Boolean) ?? null;
  } catch {
    return null;
  }
}

function ffmpegInstall(): { command: string; url: string; manager: string } {
  return process.platform === "win32"
    ? { command: "winget install --id Gyan.FFmpeg", url: "https://www.gyan.dev/ffmpeg/builds/", manager: "WinGet" }
    : { command: "brew install ffmpeg", url: "https://formulae.brew.sh/formula/ffmpeg", manager: "Homebrew" };
}

function getDimensions(filePath: string): { width: number; height: number } {
  const ffprobe = findBinary("ffprobe");
  if (!ffprobe)
    throw new Error(`ffprobe not found — install with: ${ffmpegInstall().command}`);

  const out = execFileSync(ffprobe, ["-v", "quiet", "-print_format", "json", "-show_streams", filePath], {
    encoding: "utf-8",
    windowsHide: true,
  });
  const streams = JSON.parse(out).streams;
  const video = streams.find(
    (s: { width?: number; height?: number }) => s.width && s.height,
  );
  if (!video) throw new Error("Could not read dimensions");
  return { width: video.width, height: video.height };
}

function getPixelData(
  filePath: string,
  w: number,
  h: number,
): Uint8ClampedArray {
  const ffmpeg = findBinary("ffmpeg");
  if (!ffmpeg)
    throw new Error(`ffmpeg not found — install with: ${ffmpegInstall().command}`);

  const buf = execFileSync(
    ffmpeg,
    ["-y", "-i", filePath, "-vframes", "1", "-s", `${w}x${h}`, "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"],
    { maxBuffer: 50 * 1024 * 1024, windowsHide: true },
  );
  return new Uint8ClampedArray(buf);
}

interface BlurHashResult {
  hash: string;
  width: number;
  height: number;
  ratio: string;
  fileName: string;
  copyText: string;
  previewDataUri: string;
}

function generatePreview(hash: string, width: number, height: number): string {
  const ffmpeg = findBinary("ffmpeg");
  if (!ffmpeg)
    throw new Error(`ffmpeg not found — install with: ${ffmpegInstall().command}`);

  const pw = 400;
  const ph = Math.round((height / width) * pw);
  const pixels = decode(hash, pw, ph);

  // Convert raw RGBA to PNG via ffmpeg, return as base64 data URI
  const rawPath = path.join(os.tmpdir(), `blurhash-raw-${Date.now()}.rgba`);
  const pngPath = path.join(os.tmpdir(), `blurhash-png-${Date.now()}.png`);

  fs.writeFileSync(rawPath, Buffer.from(pixels.buffer));
  execFileSync(
    ffmpeg,
    ["-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", `${pw}x${ph}`, "-i", rawPath, pngPath],
    { stdio: "pipe", windowsHide: true },
  );

  const pngBuffer = fs.readFileSync(pngPath);
  fs.unlinkSync(rawPath);
  fs.unlinkSync(pngPath);

  return `data:image/png;base64,${pngBuffer.toString("base64")}`;
}

function generateBlurHash(filePath: string): BlurHashResult {
  const { width, height } = getDimensions(filePath);

  // Resize for hash computation — keep aspect ratio, max 100px
  const scale = Math.min(100 / width, 100 / height, 1);
  const sw = Math.round(width * scale);
  const sh = Math.round(height * scale);

  const pixels = getPixelData(filePath, sw, sh);
  const hash = encode(pixels, sw, sh, 4, 3);

  const previewDataUri = generatePreview(hash, width, height);

  const fileName = path.basename(filePath);
  const ratio = aspectRatio(width, height);
  const copyText = `${fileName} ${width}x${height} ${ratio} ${hash}`;

  return { hash, width, height, ratio, fileName, copyText, previewDataUri };
}

function isFile(p: string): boolean {
  try {
    return fs.existsSync(p) && fs.statSync(p).isFile();
  } catch {
    return false;
  }
}

function hasSupportedExt(p: string): boolean {
  return SUPPORTED_EXTS.has(path.extname(p).toLowerCase());
}

function resolveClipboardFile(clip: {
  file?: string;
  text?: string;
}): string | null {
  // Method 1: Raycast clipboard file property (trust it — can be extensionless temp files)
  if (clip.file) {
    const f = decodeURIComponent(clip.file.replace(/^file:\/\//, ""));
    if (isFile(f)) return f;
  }

  // Method 2: Clipboard text is a file path
  if (clip.text) {
    const text = clip.text.trim().replace(/^file:\/\//, "");
    const decoded = decodeURIComponent(text);
    if (isFile(decoded) && hasSupportedExt(decoded)) return decoded;
    if (isFile(text) && hasSupportedExt(text)) return text;
  }

  return null;
}

function ResultView({ result }: { result: BlurHashResult }) {
  const markdown = `\n\n\n\n![preview](${result.previewDataUri})`;

  return (
    <Detail
      markdown={markdown}
      metadata={
        <Detail.Metadata>
          <Detail.Metadata.Label title="File" text={result.fileName} />
          <Detail.Metadata.Label
            title="Dimensions"
            text={`${result.width} × ${result.height}`}
          />
          <Detail.Metadata.Label title="Ratio" text={result.ratio} />
          <Detail.Metadata.Separator />
          <Detail.Metadata.Label title="BlurHash" text={result.hash} />
        </Detail.Metadata>
      }
      actions={
        <ActionPanel>
          <Action.CopyToClipboard title="Copy BlurHash" content={result.hash} />
          <Action.CopyToClipboard title="Copy All" content={result.copyText} />
        </ActionPanel>
      }
    />
  );
}

function FallbackMenu({
  onResult,
}: {
  onResult: (result: BlurHashResult) => void;
}) {
  return (
    <List>
      <List.EmptyView
        icon={Icon.Image}
        title="No image found in clipboard"
        description="Select a file in Finder or File Explorer, or copy a supported file"
      />
      <List.Item
        icon={Icon.Clipboard}
        title="Open Clipboard History"
        subtitle="Re-copy a file, then try again"
        actions={
          <ActionPanel>
            <Action
              title="Open Clipboard History"
              onAction={() =>
                open(
                  `${process.env.RAYCAST_SCHEME ?? "raycast"}://extensions/raycast/clipboard-history/clipboard-history`,
                )
              }
            />
          </ActionPanel>
        }
      />
      <List.Item
        icon={Icon.Document}
        title="Use Selected File"
        subtitle="Read the image or video selected in Finder or File Explorer"
        actions={
          <ActionPanel>
            <Action
              title="Generate from Selected File"
              onAction={async () => {
                try {
                  const selected = await getSelectedFinderItems();
                  const filePath = selected.find((item) => isFile(item.path) && hasSupportedExt(item.path))?.path;
                  if (!filePath) throw new Error("Select a supported image or video file first.");
                  await showToast({
                    style: Toast.Style.Animated,
                    title: "Generating BlurHash...",
                  });
                  const res = generateBlurHash(filePath);
                  onResult(res);
                  await showToast({
                    style: Toast.Style.Success,
                    title: "Done!",
                  });
                } catch (error) {
                  await showToast({
                    style: Toast.Style.Failure,
                    title: "Could not use selected file",
                    message: error instanceof Error ? error.message : String(error),
                  });
                }
              }}
            />
          </ActionPanel>
        }
      />
    </List>
  );
}

function cleanupStaleTempFiles() {
  try {
    const tmpDir = os.tmpdir();
    for (const f of fs.readdirSync(tmpDir)) {
      if (f.startsWith("blurhash-raw-") || f.startsWith("blurhash-png-")) {
        try {
          fs.unlinkSync(path.join(tmpDir, f));
        } catch {
          /* ignore */
        }
      }
    }
  } catch {
    /* ignore */
  }
}

export default function Command() {
  const [showPicker, setShowPicker] = useState(false);
  const [missingFfmpeg, setMissingFfmpeg] = useState(false);
  const [result, setResult] = useState<BlurHashResult | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    cleanupStaleTempFiles();

    if (!findBinary("ffmpeg") || !findBinary("ffprobe")) {
      setMissingFfmpeg(true);
      setIsLoading(false);
      return;
    }

    (async () => {
      try {
        await showToast({
          style: Toast.Style.Animated,
          title: "Reading clipboard...",
        });
        const clip = await Clipboard.read();
        let filePath = resolveClipboardFile(clip);
        if (!filePath) {
          const selected = await getSelectedFinderItems().catch(() => []);
          filePath = selected.find((item) => isFile(item.path) && hasSupportedExt(item.path))?.path ?? null;
        }

        if (!filePath) {
          await showToast({
            style: Toast.Style.Failure,
            title: "No image in clipboard",
          });
          setShowPicker(true);
          setIsLoading(false);
          return;
        }

        await showToast({
          style: Toast.Style.Animated,
          title: "Generating BlurHash...",
        });
        const res = generateBlurHash(filePath);
        setResult(res);
        await showToast({ style: Toast.Style.Success, title: "Done!" });
      } catch (e) {
        const msg = e instanceof Error ? e.message : "Unknown error";
        await showToast({
          style: Toast.Style.Failure,
          title: "Error",
          message: msg,
        });
        setShowPicker(true);
      } finally {
        setIsLoading(false);
      }
    })();
  }, []);

  if (missingFfmpeg) {
    const install = ffmpegInstall();
    return (
      <Detail
        markdown={`# ffmpeg not found\n\nThis extension requires ffmpeg to process images and videos.\n\nInstall it with ${install.manager}:\n\n\`\`\`\n${install.command}\n\`\`\``}
        actions={
          <ActionPanel>
            <Action.CopyToClipboard
              title="Copy Install Command"
              content={install.command}
            />
            <Action.OpenInBrowser
              title={`Open ${install.manager} Instructions`}
              url={install.url}
            />
          </ActionPanel>
        }
      />
    );
  }

  if (result) {
    return <ResultView result={result} />;
  }

  if (showPicker) {
    return (
      <FallbackMenu
        onResult={(res) => {
          setShowPicker(false);
          setResult(res);
        }}
      />
    );
  }

  return <Detail isLoading={isLoading} markdown="" />;
}
