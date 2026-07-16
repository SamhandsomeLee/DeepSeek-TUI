import Link from "next/link";
import { fetchFeed, fetchRepoStats } from "@/lib/github";
import { getDispatch, getEnv } from "@/lib/kv";
import { getFacts } from "@/lib/facts";
import { buildPageMetadata } from "@/lib/page-meta";
import { Seal } from "@/components/seal";
import { Ticker } from "@/components/ticker";
import { StatGrid } from "@/components/stat-grid";
import { RELEASE_CONTRIBUTORS, RELEASE_HELPERS } from "@/lib/release-credits";
import type { CuratedDispatch, FeedItem, RepoStats } from "@/lib/types";

export const revalidate = 1800;

const FALLBACK_STATS: RepoStats = {
  stars: 0,
  forks: 0,
  openIssues: 0,
  openPulls: 0,
  contributors: 141,
  fetchedAt: new Date().toISOString(),
};

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  return buildPageMetadata({
    path: "/community",
    locale,
    title: isZh ? "社区 · Codewhale" : "Community · Codewhale",
    description: isZh
      ? "Codewhale 的社区一角：实时仓库动态、每周摘要、路线图、贡献指南与发布致谢，集中在一处。"
      : "The community side of Codewhale in one place: live repo activity, the weekly digest, the roadmap, how to contribute, and release credits.",
  });
}

export default async function CommunityPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  const p = (path: string) => (isZh ? `/zh${path}` : path);

  const env = await getEnv();
  const facts = await getFacts();

  let stats: RepoStats = FALLBACK_STATS;
  let feed: FeedItem[] = [];
  let dispatch: CuratedDispatch | null = null;

  try {
    [stats, feed] = await Promise.all([
      fetchRepoStats(env.GITHUB_TOKEN),
      fetchFeed(env.GITHUB_TOKEN, 12),
    ]);
  } catch (e) {
    console.error("github fetch failed", e);
  }

  try {
    dispatch = await getDispatch();
  } catch {
    /* dispatch stays null; the section falls back to a link */
  }

  const highlights =
    dispatch && isZh && dispatch.highlightsZh ? dispatch.highlightsZh : dispatch?.highlights ?? [];

  const hubs = isZh
    ? [
        { t: "提交问题", d: "报告 bug、兼容性问题或不清楚的行为。请附上系统信息、复现步骤和相关日志。", cta: "提交 issue →", href: "https://github.com/Hmbown/CodeWhale/issues/new/choose", tag: "反馈" },
        { t: "发送改进", d: "修复运行时、测试或文档中的一个具体问题，并提交范围清晰、便于审查的 pull request。", cta: "阅读贡献指南 →", href: p("/contribute"), tag: "代码与文档" },
        { t: "改进翻译", d: "帮助 Codewhale 在更多语言和地区中自然、准确地工作。翻译指南列出了完整语言包与待补项目。", cta: "查看本地化指南 ↗", href: "https://github.com/Hmbown/CodeWhale/blob/main/docs/LOCALIZATION.md", tag: "语言" },
        { t: "跟进项目动态", d: "查看最近的 issues、pull requests、版本路线和经过维护者审核的社区摘要。", cta: "查看动态 →", href: p("/feed"), tag: "动态" },
      ]
    : [
        { t: "Report a problem", d: "File a bug, compatibility problem, or unclear behavior. Include your system details, reproduction steps, and relevant logs.", cta: "File an issue →", href: "https://github.com/Hmbown/CodeWhale/issues/new/choose", tag: "feedback" },
        { t: "Send an improvement", d: "Fix one concrete problem in the runtime, tests, or documentation and open a focused pull request that is easy to review.", cta: "Read the contribution guide →", href: p("/contribute"), tag: "code and docs" },
        { t: "Improve a translation", d: "Help Codewhale work naturally and accurately across more languages and regions. The localization guide lists complete packs and open gaps.", cta: "Open the localization guide ↗", href: "https://github.com/Hmbown/CodeWhale/blob/main/docs/LOCALIZATION.md", tag: "language" },
        { t: "Follow project activity", d: "Browse recent issues and pull requests, the public roadmap, and maintainer-reviewed community summaries.", cta: "Open the activity feed →", href: p("/feed"), tag: "activity" },
      ];

  return (
    <>
      <section className="community-welcome">
        <div className="portal-current" aria-hidden="true" />
        <div className="portal-container community-welcome-inner">
          <div className="eyebrow">{isZh ? "国际开源社区" : "International open-source community"}</div>
          <h1>{isZh ? "与世界各地的贡献者一起构建 Codewhale。" : "Build Codewhale with contributors around the world."}</h1>
          <p>
            {isZh
              ? "Codewhale 的代码、文档、测试和翻译由不同国家、语言和技术背景的贡献者共同改进。第一次参与不需要从大功能开始：一份清楚的 bug 报告、一处文档修正或一个带测试的小补丁都很有价值。"
              : "Codewhale's runtime, documentation, tests, and translations improve through contributors across countries, languages, and technical backgrounds. A first contribution does not need to be a large feature: a clear bug report, a documentation correction, or a small tested patch is useful project work."}
          </p>
          <div className="portal-actions">
            <Link href="https://github.com/Hmbown/CodeWhale/issues/new/choose" className="portal-button portal-button-primary">
              {isZh ? "提交 issue" : "File an issue"}
            </Link>
            <Link href="https://github.com/Hmbown/CodeWhale/pulls" className="portal-button portal-button-secondary">
              {isZh ? "查看 pull requests" : "Browse pull requests"}
            </Link>
            <Link href="https://github.com/Hmbown/CodeWhale/blob/main/docs/LOCALIZATION.md" className="portal-button portal-button-secondary">
              {isZh ? "改进翻译" : "Improve a translation"}
            </Link>
          </div>
        </div>
      </section>

      {/* live repo activity — the same ticker as the homepage, composed here */}
      <Ticker items={feed} />

      {/* Direct contribution paths */}
      <section className="mx-auto max-w-[1400px] px-6 py-12">
        <div className="mb-6 hairline-b pb-4">
          <div className="eyebrow mb-2">{isZh ? "现在就可以参与" : "Ways to contribute now"}</div>
          <h2 className="font-display">{isZh ? "选择一个具体的切入点。" : "Choose one practical place to start."}</h2>
        </div>
        <div className="grid md:grid-cols-2 gap-0 col-rule hairline-t hairline-b">
          {hubs.map((h) => (
            <Link key={h.t} href={h.href} className="block p-6 hover:bg-paper-deep transition-colors">
              <div className="flex items-baseline justify-between gap-3 mb-2">
                <h3 className="font-display text-xl">{h.t}</h3>
                <span className="font-mono text-[0.62rem] uppercase tracking-widest text-indigo shrink-0">{h.tag}</span>
              </div>
              <p className={`text-sm text-ink-soft mb-4 ${isZh ? "leading-[1.9] tracking-wide" : "leading-relaxed"}`}>
                {h.d}
              </p>
              <span className="font-mono text-[0.7rem] uppercase tracking-widest text-indigo">{h.cta}</span>
            </Link>
          ))}
        </div>
      </section>

      {/* the numbers — same StatGrid as the homepage */}
      <StatGrid stats={stats} />

      {/* TODAY'S DISPATCH — composed from the same cron-curated source */}
      <section className="bg-paper-deep hairline-t hairline-b">
        <div className="mx-auto max-w-[1400px] px-6 py-14">
          <div className="flex items-baseline gap-4 mb-8 hairline-b pb-4">
            <Seal char="讯" />
            <h2 className="font-display">{isZh ? "今日要闻" : "Today's dispatch"}</h2>
          </div>
          {dispatch ? (
            <article className="grid lg:grid-cols-12 gap-x-10 gap-y-6 max-w-[1100px]">
              <h3 className="lg:col-span-12 font-display text-2xl sm:text-3xl leading-tight">
                {isZh && dispatch.headlineZh ? dispatch.headlineZh : dispatch.headline}
              </h3>
              <p className={`lg:col-span-7 text-ink-soft ${isZh ? "leading-[1.9] tracking-wide" : "leading-relaxed"}`}>
                {isZh && dispatch.summaryZh ? dispatch.summaryZh : dispatch.summary}
              </p>
              <ul className="lg:col-span-5 space-y-3">
                {highlights.slice(0, 3).map((h, i) => (
                  <li key={i} className="flex items-baseline gap-3">
                    <span className="font-mono text-[0.66rem] text-indigo uppercase tracking-widest w-16 shrink-0">{h.tag}</span>
                    <div>
                      <Link href={h.href} className="body-link font-display text-base leading-snug">
                        {h.title}
                      </Link>
                      <p className={`text-sm text-ink-soft mt-0.5 ${isZh ? "leading-[1.8]" : ""}`}>{h.blurb}</p>
                    </div>
                  </li>
                ))}
              </ul>
            </article>
          ) : (
            <p className={`text-ink-soft max-w-2xl ${isZh ? "leading-[1.9] tracking-wide" : "leading-relaxed"}`}>
              {isZh ? (
                <>
                  当前社区要闻暂不可用；你仍可在{" "}
                  <Link href={p("/digest")} className="body-link">社区摘要</Link>
                  中浏览每周的仓库动态存档。
                </>
              ) : (
                <>
                  While a current dispatch is unavailable, the{" "}
                  <Link href={p("/digest")} className="body-link">community digest</Link>{" "}
                  keeps the weekly archive of repository activity.
                </>
              )}
            </p>
          )}
        </div>
      </section>

      {/* RELEASE CREDITS — the people behind the current release */}
      <section className="mx-auto max-w-[1400px] px-6 py-14">
        <div className="flex items-baseline gap-4 mb-5 hairline-b pb-4">
          <Seal char="谢" />
          <div>
            <div className="eyebrow mb-2">{isZh ? `v${facts.version} 致谢` : `v${facts.version} credits`}</div>
            <h2 className="font-display text-3xl">{isZh ? "每个补丁和报告都算数" : "Every patch and report counts"}</h2>
          </div>
        </div>
        <div className="grid lg:grid-cols-12 gap-10">
          <div className="lg:col-span-5">
            <p className={`text-ink-soft ${isZh ? "leading-[1.9] tracking-wide" : "leading-relaxed"}`}>
              {isZh
                ? "这一版本包含了社区提交的代码、测试、问题复现和审查建议。完整记录保存在 CHANGELOG 和贡献者名单中；即使补丁需要由维护者整理后合入，原作者的贡献也会保留。"
                : "This release includes code, tests, reproductions, and review guidance from the community. The changelog and contributor list keep the full record, including credit when a maintainer adapts a patch before it lands."}
            </p>
            <div className="flex flex-wrap gap-x-5 gap-y-2 mt-4">
              <Link href="https://github.com/Hmbown/CodeWhale/blob/main/docs/CONTRIBUTORS.md" className="font-mono text-xs uppercase tracking-wider text-indigo hover:underline">
                {isZh ? "完整贡献者名单 →" : "Full contributor list →"}
              </Link>
              <Link href="https://github.com/Hmbown/CodeWhale/blob/main/CHANGELOG.md" className="font-mono text-xs uppercase tracking-wider text-indigo hover:underline">
                CHANGELOG →
              </Link>
              <Link href={p("/contribute")} className="font-mono text-xs uppercase tracking-wider text-indigo hover:underline">
                {isZh ? "参与贡献 →" : "Contribute →"}
              </Link>
            </div>
          </div>
          <div className="lg:col-span-7 grid gap-6">
            <div>
              <div className="eyebrow mb-3">{isZh ? "已合并 / 已吸收贡献" : "Merged and harvested contributions"}</div>
              <div className="flex flex-wrap gap-2">
                {RELEASE_CONTRIBUTORS.map((handle) => (
                  <Link
                    key={handle}
                    href={`https://github.com/${handle.slice(1)}`}
                    className="font-mono text-xs px-2 py-1 hairline-t hairline-b hairline-l hairline-r text-ink-soft hover:text-indigo hover:bg-paper-deep"
                  >
                    {handle}
                  </Link>
                ))}
              </div>
            </div>
            <div>
              <div className="eyebrow mb-3">{isZh ? "报告、复现和验证" : "Reports, repros, and verification"}</div>
              <div className="flex flex-wrap gap-2">
                {RELEASE_HELPERS.map((handle) => (
                  <Link
                    key={handle}
                    href={`https://github.com/${handle.slice(1)}`}
                    className="font-mono text-xs px-2 py-1 hairline-t hairline-b hairline-l hairline-r text-ink-soft hover:text-indigo hover:bg-paper-deep"
                  >
                    {handle}
                  </Link>
                ))}
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* JOIN IN — one clear ask, matching the homepage closer */}
      <section className="bg-ink text-paper">
        <div className="mx-auto max-w-[1400px] px-6 py-16 grid lg:grid-cols-12 gap-10 items-center">
          <div className="lg:col-span-8">
            <div className="eyebrow text-paper-deep/70 mb-3">{isZh ? "参与其中" : "Join in"}</div>
            <h2 className="font-display text-paper text-3xl sm:text-4xl leading-tight">
              {isZh ? "告诉项目什么有效，什么还需要改进。" : "Tell the project what works and what still needs attention."}
            </h2>
            <p className={`mt-5 text-paper-deep/80 max-w-2xl ${isZh ? "leading-[1.9]" : "leading-relaxed"}`}>
              {isZh
                ? "如果你遇到 bug、需要一个尚未支持的模型，或发现文档不清楚，请提交 issue。已经有明确改进方案时，也欢迎直接发送带测试的 pull request。"
                : "If you hit a bug, need a model that is not supported yet, or find unclear documentation, file an issue. If you already have a focused improvement, a tested pull request is welcome too."}
            </p>
            <div className="mt-6 flex flex-wrap items-center gap-3">
              <Link href="https://github.com/Hmbown/CodeWhale/issues/new" className="px-4 py-2 bg-indigo text-paper font-mono text-sm hover:bg-indigo-deep transition-colors">
                {isZh ? "开个 issue →" : "Open an issue →"}
              </Link>
              <Link href={p("/contribute")} className="px-4 py-2 hairline-t hairline-b hairline-l hairline-r border-white/20 text-paper font-mono text-sm hover:bg-white/10 transition-colors">
                {isZh ? "参与贡献 →" : "Contribute →"}
              </Link>
              <Link href="https://github.com/Hmbown/CodeWhale/discussions" className="px-4 py-2 font-mono text-sm text-paper-deep/80 hover:text-paper transition-colors">
                {isZh ? "讨论区 →" : "Discussions →"}
              </Link>
            </div>
          </div>
          <div className="lg:col-span-4 font-mono text-sm text-paper-deep/80 space-y-2">
            <div className="flex justify-between hairline-b border-white/15 pb-2">
              <span className="uppercase tracking-widest text-[0.66rem] text-paper-deep/60">{isZh ? "版本" : "version"}</span>
              <span className="tabular text-paper">{facts.version ?? "v0.8.x"}</span>
            </div>
            <div className="flex justify-between hairline-b border-white/15 pb-2">
              <span className="uppercase tracking-widest text-[0.66rem] text-paper-deep/60">{isZh ? "提供商" : "providers"}</span>
              <span className="tabular text-paper">{facts.providers.length}</span>
            </div>
            <div className="flex justify-between hairline-b border-white/15 pb-2">
              <span className="uppercase tracking-widest text-[0.66rem] text-paper-deep/60">{isZh ? "许可证" : "license"}</span>
              <span className="text-paper">{facts.license ?? "MIT"}</span>
            </div>
          </div>
        </div>
      </section>
    </>
  );
}
