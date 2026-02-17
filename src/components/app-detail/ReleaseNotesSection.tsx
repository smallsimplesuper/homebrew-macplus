import { ExternalLink, FileText } from "lucide-react";
import { useMemo } from "react";
import { cn } from "@/lib/utils";

interface ReleaseNotesSectionProps {
  releaseNotesUrl: string | null;
  releaseNotes: string | null;
}

const MARKDOWN_INDICATORS = /[#*\-[`>]/;

// Pre-compiled regex patterns for markdown rendering
const RE_AMP = /&/g;
const RE_LT = /</g;
const RE_GT = />/g;
const RE_H4 = /^###\s+(.+)$/gm;
const RE_H3_SEMI = /^##\s+(.+)$/gm;
const RE_H3 = /^#\s+(.+)$/gm;
const RE_BOLD_STAR = /\*\*(.+?)\*\*/g;
const RE_BOLD_UNDER = /__(.+?)__/g;
const RE_ITALIC = /(?<!\w)\*(?!\s)(.+?)(?<!\s)\*(?!\w)/g;
const RE_CODE = /`([^`]+)`/g;
const RE_LINK = /\[([^\]]+)\]\(([^)]+)\)/g;
const RE_LIST_ITEM = /^[\s]*[-*]\s+(.+)$/gm;
const RE_WRAP_UL = /(<li[^>]*>.*?<\/li>\n?)+/g;
const RE_NEWLINE = /\n/g;
const RE_BR_AFTER_H = /(<\/h[34]>)<br \/>/g;
const RE_BR_AFTER_UL = /(<\/ul>)<br \/>/g;
const RE_BR_AFTER_LI = /(<\/li>)<br \/>/g;

const ALLOWED_TAGS = /^<\/?(h[34]|strong|em|code|a|ul|li|br)\b[^>]*>$/i;

function hasMarkdown(text: string): boolean {
  return MARKDOWN_INDICATORS.test(text);
}

function sanitizeHtml(html: string): string {
  return html.replace(/<\/?[a-z][^>]*>/gi, (tag) => (ALLOWED_TAGS.test(tag) ? tag : ""));
}

function renderMarkdown(text: string): string {
  let html = text.replace(RE_AMP, "&amp;").replace(RE_LT, "&lt;").replace(RE_GT, "&gt;");

  html = html.replace(RE_H4, '<h4 class="text-xs font-semibold mt-2 mb-0.5">$1</h4>');
  html = html.replace(RE_H3_SEMI, '<h3 class="text-sm font-semibold mt-2 mb-0.5">$1</h3>');
  html = html.replace(RE_H3, '<h3 class="text-sm font-bold mt-2 mb-0.5">$1</h3>');
  html = html.replace(RE_BOLD_STAR, "<strong>$1</strong>");
  html = html.replace(RE_BOLD_UNDER, "<strong>$1</strong>");
  html = html.replace(RE_ITALIC, "<em>$1</em>");
  html = html.replace(
    RE_CODE,
    '<code class="rounded bg-muted px-1 py-0.5 text-footnote">$1</code>',
  );
  html = html.replace(
    RE_LINK,
    '<a href="$2" target="_blank" rel="noopener noreferrer" class="text-primary hover:underline">$1</a>',
  );
  html = html.replace(RE_LIST_ITEM, '<li class="ml-3 list-disc text-xs leading-relaxed">$1</li>');
  html = html.replace(RE_WRAP_UL, '<ul class="my-1 space-y-0.5">$&</ul>');
  html = html.replace(RE_NEWLINE, "<br />");
  html = html.replace(RE_BR_AFTER_H, "$1");
  html = html.replace(RE_BR_AFTER_UL, "$1");
  html = html.replace(RE_BR_AFTER_LI, "$1");

  return sanitizeHtml(html);
}

export function ReleaseNotesContent({ releaseNotes, releaseNotesUrl }: ReleaseNotesSectionProps) {
  const renderedHtml = useMemo(
    () => (releaseNotes && hasMarkdown(releaseNotes) ? renderMarkdown(releaseNotes) : null),
    [releaseNotes],
  );

  if (releaseNotes) {
    return (
      <div className="space-y-2">
        {renderedHtml ? (
          <div
            className="max-h-40 overflow-y-auto text-xs text-foreground leading-relaxed"
            // biome-ignore lint/security/noDangerouslySetInnerHtml: sanitized markdown rendering
            dangerouslySetInnerHTML={{ __html: renderedHtml }}
          />
        ) : (
          <div className="max-h-40 overflow-y-auto whitespace-pre-wrap text-xs text-foreground leading-relaxed">
            {releaseNotes}
          </div>
        )}
        {releaseNotesUrl && (
          <a
            href={releaseNotesUrl}
            target="_blank"
            rel="noopener noreferrer"
            className={cn(
              "flex items-center gap-1.5 text-xs text-primary",
              "transition-colors hover:text-primary/80 hover:underline",
            )}
          >
            <ExternalLink className="h-3 w-3" />
            View full release notes
          </a>
        )}
      </div>
    );
  }

  if (releaseNotesUrl) {
    return (
      <a
        href={releaseNotesUrl}
        target="_blank"
        rel="noopener noreferrer"
        className={cn(
          "flex items-center gap-2 text-sm text-primary",
          "transition-colors hover:text-primary/80 hover:underline",
        )}
      >
        <ExternalLink className="h-3.5 w-3.5" />
        View release notes
      </a>
    );
  }

  return (
    <div className="flex items-center gap-2 text-muted-foreground">
      <FileText className="h-3.5 w-3.5" />
      <span className="text-xs">No release notes available</span>
    </div>
  );
}

export function ReleaseNotesSection({ releaseNotesUrl, releaseNotes }: ReleaseNotesSectionProps) {
  return (
    <div className="space-y-1">
      <h4 className="text-caption-uppercase tracking-wider text-muted-foreground">Release Notes</h4>
      <div className="rounded-lg border border-border bg-background p-3">
        <ReleaseNotesContent releaseNotes={releaseNotes} releaseNotesUrl={releaseNotesUrl} />
      </div>
    </div>
  );
}
