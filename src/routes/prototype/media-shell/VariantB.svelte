<script lang="ts">
  import {
    ArrowLeft,
    BookOpen,
    ChevronDown,
    Compass,
    Disc3,
    LibraryBig,
    Moon,
    Play,
    Radio,
    Sun,
    Workflow,
    X,
  } from '@lucide/svelte';
  import { prefersReducedMotion } from 'svelte/motion';
  import { blur, fade, fly } from 'svelte/transition';
  import ContinuousMediaBar from './ContinuousMediaBar.svelte';
  import CoverArt from './CoverArt.svelte';
  import {
    albums,
    novelShelf,
    readerParagraphs,
    realmStories,
    spaceOptions,
    tracks,
    type PrototypeShellProps,
  } from './prototype-fixtures';

  let {
    space,
    theme,
    audioState,
    onspacechange,
    onthemechange,
    onaudiochange,
    onqueueopen,
  }: PrototypeShellProps = $props();

  let portalOpen = $state(false);
  const activeLabel = $derived(spaceOptions.find((item) => item.id === space)?.label ?? '境场');

  function selectSpace(next: 'realm' | 'novel' | 'music' | 'reader'): void {
    onspacechange(next);
    portalOpen = false;
  }
</script>

<div class="variant-b" data-space={space}>
  <div class="b-atmosphere" aria-hidden="true"></div>

  {#if space === 'reader'}
    <section class="b-reader">
      <header>
        <button class="portal-trigger compact" type="button" onclick={() => (portalOpen = true)}>
          <span class="frame-mark" aria-hidden="true"><i></i><i></i></span>
          <span>览境</span>
          <span class="compact-chevron" aria-hidden="true">
            <ChevronDown size={14} strokeWidth={1.5} />
          </span>
        </button>
        <div class="b-reader-location"><span>小说空间</span><strong>在潮汐线醒来 · 18</strong></div>
        <button
          class="round-button"
          type="button"
          onclick={() => onthemechange(theme === 'dark' ? 'light' : 'dark')}
          aria-label="切换主题"
        >
          {#if theme === 'dark'}<Sun size={17} strokeWidth={1.5} aria-hidden="true" />{:else}<Moon
              size={17}
              strokeWidth={1.5}
              aria-hidden="true"
            />{/if}
        </button>
      </header>
      <main>
        <button class="reader-back" type="button" onclick={() => onspacechange('novel')}>
          <ArrowLeft size={16} strokeWidth={1.5} aria-hidden="true" />返回活书架
        </button>
        <article>
          <span>第 18 章</span>
          <h1>风暴将至</h1>
          {#each readerParagraphs as paragraph, index (`b-${index}`)}<p>{paragraph}</p>{/each}
        </article>
        <aside class="b-reader-margin"><span>62</span><i></i><small>100</small></aside>
        {#if audioState !== 'none'}
          <div class="b-reader-audio">
            <ContinuousMediaBar
              state={audioState}
              tone="B"
              reader
              onstatechange={onaudiochange}
              {onqueueopen}
            />
          </div>
        {:else}
          <button class="b-reader-restore" type="button" onclick={() => onaudiochange('paused')}
            >恢复环境音频</button
          >
        {/if}
      </main>
    </section>
  {:else}
    <section class="b-shell">
      <header class="b-header">
        <button
          class="portal-trigger"
          type="button"
          onclick={() => (portalOpen = true)}
          aria-expanded={portalOpen}
        >
          <span class="frame-mark" aria-hidden="true"><i></i><i></i></span>
          <span><small>LanJing 门户</small><strong>{activeLabel}空间</strong></span>
          <ChevronDown size={15} strokeWidth={1.5} aria-hidden="true" />
        </button>

        <nav class="b-local-nav" aria-label="媒体空间切换">
          <button
            type="button"
            class:active={space === 'realm'}
            onclick={() => onspacechange('realm')}>境场</button
          >
          <button
            type="button"
            class:active={space === 'novel'}
            onclick={() => onspacechange('novel')}>小说</button
          >
          <button
            type="button"
            class:active={space === 'music'}
            onclick={() => onspacechange('music')}>音乐</button
          >
        </nav>

        <div class="b-header-actions">
          {#if audioState === 'none'}<button
              class="audio-restore"
              type="button"
              onclick={() => onaudiochange('paused')}>恢复环境音频</button
            >{/if}
          <button
            class="round-button"
            type="button"
            onclick={() => onthemechange(theme === 'dark' ? 'light' : 'dark')}
            aria-label="切换主题"
          >
            {#if theme === 'dark'}<Sun size={17} strokeWidth={1.5} aria-hidden="true" />{:else}<Moon
                size={17}
                strokeWidth={1.5}
                aria-hidden="true"
              />{/if}
          </button>
          <div class="window-dots" aria-hidden="true"><i></i><i></i><i></i></div>
        </div>
      </header>

      <main class="b-stage">
        {#key space}
          <div
            class="b-scene"
            in:blur={{
              amount: prefersReducedMotion.current ? 0 : 8,
              duration: prefersReducedMotion.current ? 0 : 520,
            }}
          >
            {#if space === 'realm'}
              <section class="b-realm">
                <div class="b-realm-intro">
                  <span>REALM / 07.14</span>
                  <h1>内容汇入<br />同一片视野。</h1>
                  <p>跨媒体发现保持中性。进入空间后，排版、密度与消费控件由媒体类型接管。</p>
                </div>
                <div class="b-realm-ribbon">
                  {#each realmStories as story, index (story.id)}
                    <button
                      type="button"
                      class:lifted={index === 1}
                      onclick={() =>
                        selectSpace(
                          story.kind === '音乐'
                            ? 'music'
                            : story.kind === '小说'
                              ? 'novel'
                              : 'realm',
                        )}
                    >
                      <CoverArt
                        title={story.title}
                        creator={story.creator}
                        kicker={story.kind}
                        art={story.art}
                        motif={story.motif}
                        foreground={story.foreground}
                        ratio={index === 0 || index === 3 ? 'landscape' : 'portrait'}
                      />
                      <span><strong>{story.title}</strong><small>{story.note}</small></span>
                    </button>
                  {/each}
                </div>
                <div class="b-realm-facts">
                  <span>最近活动 04</span><span>固定空间 02</span><span>来源范围 全部</span>
                </div>
              </section>
            {:else if space === 'novel'}
              <section class="b-novel">
                <div class="b-novel-heading">
                  <span>NOVEL SPACE</span>
                  <h1>活书架</h1>
                  <p>阅读进度决定视觉重心。书架不是资料库副本。</p>
                  <div class="b-novel-tabs">
                    <button class="active" type="button">活书架</button><button type="button"
                      >发现</button
                    >
                  </div>
                </div>
                <button
                  class="b-featured-book"
                  type="button"
                  onclick={() => onspacechange('reader')}
                >
                  <CoverArt
                    title={novelShelf[0].title}
                    creator={novelShelf[0].creator}
                    kicker="继续阅读"
                    art={novelShelf[0].art}
                    motif={novelShelf[0].motif}
                    foreground={novelShelf[0].foreground}
                  />
                  <span class="b-featured-copy">
                    <small>62% · 昨晚阅读</small>
                    <strong>{novelShelf[0].title}</strong>
                    <span>{novelShelf[0].chapter}</span>
                    <i><b></b></i>
                  </span>
                </button>
                <div class="b-book-stack">
                  {#each novelShelf.slice(1) as book (book.id)}
                    <button type="button">
                      <CoverArt
                        title={book.title}
                        creator={book.creator}
                        kicker={book.kicker}
                        art={book.art}
                        motif={book.motif}
                        foreground={book.foreground}
                      />
                      <span><strong>{book.title}</strong><small>{book.creator}</small></span>
                    </button>
                  {/each}
                </div>
              </section>
            {:else}
              <section class="b-music">
                <div class="b-record-stage">
                  <CoverArt
                    title={albums[0].title}
                    creator={albums[0].creator}
                    kicker="ALBUM 01"
                    art={albums[0].art}
                    motif={albums[0].motif}
                    foreground={albums[0].foreground}
                    ratio="square"
                    quiet
                  />
                  <div
                    class="b-record"
                    class:spinning={audioState === 'playing'}
                    aria-hidden="true"
                  >
                    <i></i>
                  </div>
                </div>
                <div class="b-album-copy">
                  <span>MUSIC SPACE · {albums[0].year}</span>
                  <h1>{albums[0].title}</h1>
                  <p>{albums[0].creator} / {albums[0].mood}</p>
                  <button class="b-play" type="button" onclick={() => onaudiochange('paused')}
                    ><Play
                      size={17}
                      fill="currentColor"
                      strokeWidth={1.3}
                      aria-hidden="true"
                    />播放专辑</button
                  >
                </div>
                <ol class="b-track-list">
                  {#each tracks as track, index (track.id)}
                    <li class:current={index === 0}>
                      <span>{index + 1}</span>
                      <p><strong>{track.title}</strong><small>{track.artist}</small></p>
                      <time>{track.duration}</time>
                    </li>
                  {/each}
                </ol>
              </section>
            {/if}
          </div>
        {/key}
      </main>

      <ContinuousMediaBar state={audioState} tone="B" onstatechange={onaudiochange} {onqueueopen} />

      <nav class="b-mobile-nav" aria-label="移动端空间导航">
        <button
          type="button"
          class:active={space === 'realm'}
          onclick={() => onspacechange('realm')}
          ><Compass size={18} strokeWidth={1.5} aria-hidden="true" /><span>境场</span></button
        >
        <button
          type="button"
          class:active={space === 'novel'}
          onclick={() => onspacechange('novel')}
          ><BookOpen size={18} strokeWidth={1.5} aria-hidden="true" /><span>小说</span></button
        >
        <button
          type="button"
          class:active={space === 'music'}
          onclick={() => onspacechange('music')}
          ><Disc3 size={18} strokeWidth={1.5} aria-hidden="true" /><span>音乐</span></button
        >
        <button type="button" onclick={() => (portalOpen = true)}
          ><ChevronDown size={18} strokeWidth={1.5} aria-hidden="true" /><span>门户</span></button
        >
      </nav>
    </section>
  {/if}

  {#if portalOpen}
    <button
      class="portal-backdrop"
      type="button"
      aria-label="关闭门户"
      onclick={() => (portalOpen = false)}
      transition:fade={{ duration: prefersReducedMotion.current ? 0 : 180 }}
    ></button>
    <div
      class="portal-panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="portal-title"
      transition:fly={{
        y: prefersReducedMotion.current ? 0 : 22,
        duration: prefersReducedMotion.current ? 0 : 380,
      }}
    >
      <header>
        <span
          ><small>LANJING</small>
          <h2 id="portal-title">去往另一片境场</h2></span
        ><button type="button" onclick={() => (portalOpen = false)} aria-label="关闭门户"
          ><X size={18} strokeWidth={1.5} /></button
        >
      </header>
      <div class="portal-primary">
        <button type="button" onclick={() => selectSpace('realm')}
          ><Compass size={20} strokeWidth={1.45} /><span
            ><strong>境场</strong><small>跨媒体发现</small></span
          ></button
        >
        <button type="button" onclick={() => selectSpace('novel')}
          ><BookOpen size={20} strokeWidth={1.45} /><span
            ><strong>小说空间</strong><small>活书架与阅读</small></span
          ></button
        >
        <button type="button" onclick={() => selectSpace('music')}
          ><Disc3 size={20} strokeWidth={1.45} /><span
            ><strong>音乐空间</strong><small>专辑、队列与播放</small></span
          ></button
        >
      </div>
      <div class="portal-secondary">
        <button type="button" disabled><LibraryBig size={18} strokeWidth={1.45} />资料库</button>
        <button type="button" disabled><Radio size={18} strokeWidth={1.45} />来源</button>
        <button type="button" disabled><Workflow size={18} strokeWidth={1.45} />规则工作室</button>
      </div>
      <footer>全局入口收敛为门户；媒体空间接管主壳。</footer>
    </div>
  {/if}
</div>

<style>
  button {
    border: 0;
    font: inherit;
  }
  h1,
  h2,
  p {
    margin: 0;
  }

  .variant-b {
    position: relative;
    isolation: isolate;
    height: 100%;
    overflow: hidden;
    background: var(--proto-canvas);
    color: var(--proto-ink);
  }
  .b-atmosphere {
    position: absolute;
    inset: -20%;
    z-index: -1;
    opacity: 0.5;
    transform: scale(1.08);
    transition:
      background 700ms cubic-bezier(0.2, 0.9, 0.1, 1),
      opacity 500ms cubic-bezier(0.32, 0.72, 0, 1);
  }
  .variant-b[data-space='realm'] .b-atmosphere {
    background:
      radial-gradient(circle at 72% 28%, rgb(93 121 125 / 0.22), transparent 30%),
      radial-gradient(circle at 18% 70%, rgb(195 104 63 / 0.12), transparent 28%);
  }
  .variant-b[data-space='novel'] .b-atmosphere {
    background: radial-gradient(circle at 70% 36%, rgb(158 119 83 / 0.16), transparent 34%);
  }
  .variant-b[data-space='music'] .b-atmosphere {
    background:
      radial-gradient(circle at 28% 46%, rgb(166 69 48 / 0.25), transparent 34%),
      radial-gradient(circle at 72% 35%, rgb(45 69 86 / 0.24), transparent 34%);
  }

  .b-shell {
    display: grid;
    grid-template-rows: 66px minmax(0, 1fr) auto auto;
    height: 100%;
    min-height: 0;
  }
  .b-header {
    display: grid;
    grid-template-columns: minmax(190px, 1fr) auto minmax(190px, 1fr);
    align-items: center;
    padding: 8px 14px;
  }
  .portal-trigger {
    display: inline-grid;
    grid-template-columns: 34px minmax(0, auto) 18px;
    gap: 9px;
    align-items: center;
    justify-self: start;
    min-height: 50px;
    padding: 5px 11px 5px 7px;
    border: 1px solid color-mix(in oklab, var(--proto-ink) 10%, transparent);
    border-radius: 17px;
    background: color-mix(in oklab, var(--proto-surface-strong) 76%, transparent);
    color: inherit;
    cursor: pointer;
    box-shadow: 0 1px 0 rgb(255 255 255 / 0.08) inset;
    backdrop-filter: blur(16px);
  }
  .portal-trigger > span:nth-child(2) {
    display: grid;
    text-align: left;
  }
  .portal-trigger small {
    color: var(--proto-muted);
    font-size: 0.55rem;
    letter-spacing: 0.06em;
  }
  .portal-trigger strong {
    font-size: 0.75rem;
    font-weight: 560;
  }
  .frame-mark {
    position: relative;
    display: block;
    width: 31px;
    height: 31px;
    color: var(--proto-accent);
  }
  .frame-mark i {
    position: absolute;
    width: 19px;
    height: 24px;
    border: 1.5px solid currentColor;
  }
  .frame-mark i:first-child {
    top: 1px;
    left: 2px;
    border-right-color: transparent;
  }
  .frame-mark i:last-child {
    right: 2px;
    bottom: 1px;
    border-left-color: transparent;
  }
  .b-local-nav {
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: 14px;
    background: color-mix(in oklab, var(--proto-ink) 5%, transparent);
  }
  .b-local-nav button {
    min-height: 34px;
    padding: 0 16px;
    border-radius: 10px;
    background: transparent;
    color: var(--proto-muted);
    cursor: pointer;
  }
  .b-local-nav button.active {
    background: var(--proto-surface-strong);
    color: var(--proto-ink);
    box-shadow: 0 1px 0 rgb(255 255 255 / 0.06) inset;
  }
  .b-header-actions {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-self: end;
  }
  .round-button,
  .portal-panel header button {
    display: grid;
    width: 38px;
    place-items: center;
    border-radius: 12px;
    background: color-mix(in oklab, var(--proto-surface-strong) 78%, transparent);
    color: inherit;
    cursor: pointer;
    aspect-ratio: 1;
  }
  .audio-restore {
    min-height: 36px;
    padding: 0 12px;
    border-radius: 11px;
    background: var(--proto-accent-soft);
    color: var(--proto-accent);
    font-size: 0.66rem;
    cursor: pointer;
  }
  .window-dots {
    display: flex;
    gap: 6px;
    margin-left: 5px;
  }
  .window-dots i {
    width: 7px;
    border-radius: 50%;
    background: color-mix(in oklab, var(--proto-ink) 22%, transparent);
    aspect-ratio: 1;
  }
  .window-dots i:last-child {
    background: var(--proto-accent);
  }

  .b-stage {
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .b-scene {
    min-height: 100%;
  }
  .b-realm {
    display: grid;
    grid-template-columns: minmax(250px, 0.72fr) minmax(540px, 1.6fr);
    grid-template-rows: 1fr auto;
    gap: 24px clamp(34px, 6vw, 90px);
    align-items: center;
    width: min(1320px, 100%);
    min-height: 100%;
    margin: auto;
    padding: clamp(34px, 5vw, 72px);
  }
  .b-realm-intro > span,
  .b-novel-heading > span,
  .b-album-copy > span {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.62rem;
    letter-spacing: 0.11em;
  }
  .b-realm-intro h1 {
    margin-top: 15px;
    font-size: clamp(3rem, 5.4vw, 6rem);
    font-weight: 500;
    line-height: 0.92;
    letter-spacing: -0.065em;
  }
  .b-realm-intro p {
    max-width: 36ch;
    margin-top: 20px;
    color: var(--proto-muted);
    font-size: 0.82rem;
    line-height: 1.72;
  }
  .b-realm-ribbon {
    display: grid;
    grid-template-columns: 1.35fr 0.75fr 0.82fr;
    gap: 12px;
    height: min(510px, 58vh);
  }
  .b-realm-ribbon > button {
    position: relative;
    min-width: 0;
    padding: 0;
    border-radius: 20px;
    background: transparent;
    color: #f4eee5;
    text-align: left;
    cursor: pointer;
    transition: transform 460ms cubic-bezier(0.2, 0.9, 0.1, 1);
  }
  .b-realm-ribbon > button.lifted {
    transform: translateY(-22px);
  }
  .b-realm-ribbon > button:hover {
    transform: translateY(-8px);
  }
  .b-realm-ribbon > button.lifted:hover {
    transform: translateY(-30px);
  }
  .b-realm-ribbon :global(.cover-art) {
    width: 100%;
    height: 100%;
    aspect-ratio: auto;
  }
  .b-realm-ribbon > button > span {
    position: absolute;
    right: 14px;
    bottom: 14px;
    left: 14px;
    display: grid;
    text-shadow: 0 2px 18px rgb(0 0 0 / 0.5);
  }
  .b-realm-ribbon > button > span strong {
    font-size: 0.76rem;
  }
  .b-realm-ribbon > button > span small {
    margin-top: 2px;
    font-size: 0.6rem;
    opacity: 0.72;
  }
  .b-realm-facts {
    grid-column: 1/-1;
    display: flex;
    gap: 22px;
    color: var(--proto-muted);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
  }

  .b-novel {
    display: grid;
    grid-template-columns: minmax(190px, 0.55fr) minmax(260px, 0.9fr) minmax(320px, 1.25fr);
    gap: clamp(24px, 4vw, 64px);
    align-items: center;
    width: min(1240px, 100%);
    min-height: 100%;
    margin: auto;
    padding: clamp(30px, 5vw, 70px);
  }
  .b-novel-heading h1,
  .b-album-copy h1 {
    margin-top: 10px;
    font-size: clamp(3.4rem, 6vw, 7rem);
    font-weight: 500;
    line-height: 0.88;
    letter-spacing: -0.07em;
  }
  .b-novel-heading p {
    max-width: 25ch;
    margin-top: 18px;
    color: var(--proto-muted);
    font-size: 0.8rem;
    line-height: 1.65;
  }
  .b-novel-tabs {
    display: flex;
    gap: 4px;
    margin-top: 24px;
  }
  .b-novel-tabs button {
    min-height: 34px;
    padding: 0 13px;
    border-radius: 999px;
    background: transparent;
    color: var(--proto-muted);
    cursor: pointer;
  }
  .b-novel-tabs button.active {
    background: var(--proto-ink);
    color: var(--proto-canvas);
  }
  .b-featured-book {
    position: relative;
    padding: 10px;
    border: 1px solid var(--proto-line);
    border-radius: 26px;
    background: color-mix(in oklab, var(--proto-surface-strong) 72%, transparent);
    color: inherit;
    text-align: left;
    cursor: pointer;
    box-shadow: 0 28px 84px rgb(9 10 13 / 0.18);
  }
  .b-featured-copy {
    position: absolute;
    right: 26px;
    bottom: 24px;
    left: 26px;
    display: grid;
    color: #f5eee5;
    text-shadow: 0 2px 18px rgb(0 0 0 / 0.5);
  }
  .b-featured-copy small {
    font-size: 0.62rem;
    opacity: 0.74;
  }
  .b-featured-copy strong {
    margin-top: 5px;
    font-size: 1.25rem;
  }
  .b-featured-copy > span {
    margin-top: 3px;
    font-size: 0.68rem;
  }
  .b-featured-copy i {
    position: relative;
    height: 2px;
    margin-top: 12px;
    overflow: hidden;
    background: rgb(255 255 255 / 0.25);
  }
  .b-featured-copy b {
    position: absolute;
    inset: 0;
    background: #e06d45;
    transform: scaleX(0.62);
    transform-origin: left;
  }
  .b-book-stack {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    align-items: end;
  }
  .b-book-stack button {
    min-width: 0;
    padding: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .b-book-stack button:nth-child(2) {
    transform: translateY(-30px);
  }
  .b-book-stack button > span {
    display: grid;
    margin-top: 10px;
  }
  .b-book-stack strong {
    overflow: hidden;
    font-size: 0.72rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .b-book-stack small {
    margin-top: 2px;
    color: var(--proto-muted);
    font-size: 0.6rem;
  }

  .b-music {
    display: grid;
    grid-template-columns: minmax(300px, 0.85fr) minmax(240px, 0.65fr) minmax(330px, 1fr);
    gap: clamp(28px, 5vw, 76px);
    align-items: center;
    width: min(1260px, 100%);
    min-height: 100%;
    margin: auto;
    padding: clamp(32px, 5vw, 72px);
  }
  .b-record-stage {
    position: relative;
  }
  .b-record-stage :global(.cover-art) {
    width: min(500px, 100%);
  }
  .b-record {
    position: absolute;
    right: -20%;
    bottom: -8%;
    width: 48%;
    border: 1px solid rgb(255 255 255 / 0.18);
    border-radius: 50%;
    background: repeating-radial-gradient(circle, #191a1e 0 3px, #25262a 4px 5px);
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.34);
    aspect-ratio: 1;
  }
  .b-record i {
    position: absolute;
    inset: 42%;
    border-radius: 50%;
    background: var(--proto-accent);
  }
  @media (prefers-reduced-motion: no-preference) {
    .b-record.spinning {
      animation: spin-record 9s linear infinite;
    }
  }
  .b-album-copy p {
    margin-top: 14px;
    color: var(--proto-muted);
    font-size: 0.78rem;
  }
  .b-play {
    display: inline-flex;
    min-height: 46px;
    align-items: center;
    gap: 9px;
    margin-top: 24px;
    padding: 0 20px;
    border-radius: 999px;
    background: var(--proto-accent);
    color: var(--proto-on-accent);
    cursor: pointer;
  }
  .b-track-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .b-track-list li {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    min-height: 58px;
    border-bottom: 1px solid var(--proto-line);
    color: var(--proto-muted);
  }
  .b-track-list li.current {
    color: var(--proto-accent);
  }
  .b-track-list li > span,
  .b-track-list time {
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
  }
  .b-track-list p {
    display: grid;
  }
  .b-track-list strong {
    color: var(--proto-ink);
    font-size: 0.76rem;
    font-weight: 560;
  }
  .b-track-list small {
    margin-top: 2px;
    font-size: 0.6rem;
  }

  .b-mobile-nav {
    display: none;
  }
  .b-reader {
    height: 100%;
    background: var(--reader-surface);
    color: var(--reader-ink);
  }
  .b-reader > header {
    position: relative;
    z-index: 5;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    height: 62px;
    padding: 8px 14px;
  }
  .portal-trigger.compact {
    grid-template-columns: 30px auto 16px;
    min-height: 44px;
  }
  .portal-trigger.compact .frame-mark {
    width: 27px;
    height: 27px;
  }
  .b-reader-location {
    display: grid;
    justify-items: center;
    font-size: 0.62rem;
  }
  .b-reader-location span {
    color: color-mix(in oklab, var(--reader-ink) 55%, transparent);
  }
  .b-reader-location strong {
    font-weight: 560;
  }
  .b-reader > header .round-button {
    justify-self: end;
  }
  .b-reader main {
    position: relative;
    height: calc(100% - 62px);
    overflow: auto;
    padding: clamp(50px, 8vh, 100px) 24px 150px;
  }
  .reader-back {
    position: fixed;
    top: 84px;
    left: 22px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    color: color-mix(in oklab, var(--reader-ink) 62%, transparent);
    cursor: pointer;
  }
  .b-reader article {
    width: min(660px, 100%);
    margin: auto;
    font-family: var(--font-reader, serif);
  }
  .b-reader article > span {
    display: block;
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.65rem;
    text-align: center;
  }
  .b-reader article h1 {
    margin: 12px 0 48px;
    font-family: var(--font-reader, serif);
    font-size: clamp(2.2rem, 4vw, 3.4rem);
    font-weight: 600;
    text-align: center;
  }
  .b-reader article p {
    margin-bottom: 1.2em;
    font-size: clamp(1.03rem, 1.4vw, 1.18rem);
    line-height: 1.88;
  }
  .b-reader-margin {
    position: fixed;
    top: 50%;
    right: 28px;
    display: grid;
    justify-items: center;
    gap: 8px;
    color: color-mix(in oklab, var(--reader-ink) 44%, transparent);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
    transform: translateY(-50%);
  }
  .b-reader-margin i {
    width: 1px;
    height: 72px;
    background: linear-gradient(
      var(--proto-accent) 62%,
      color-mix(in oklab, var(--reader-ink) 12%, transparent) 62%
    );
  }
  .b-reader-audio {
    position: fixed;
    right: 24px;
    bottom: 24px;
  }
  .b-reader-restore {
    position: fixed;
    right: 24px;
    bottom: 24px;
    min-height: 42px;
    padding: 0 16px;
    border-radius: 999px;
    background: var(--proto-accent-soft);
    color: var(--proto-accent);
    cursor: pointer;
  }

  .portal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgb(6 7 9 / 0.58);
    backdrop-filter: blur(8px);
  }
  .portal-panel {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 61;
    width: min(660px, calc(100vw - 32px));
    padding: 22px;
    border: 1px solid color-mix(in oklab, var(--proto-ink) 12%, transparent);
    border-radius: 26px;
    background: var(--proto-surface-strong);
    color: var(--proto-ink);
    box-shadow: 0 30px 100px rgb(0 0 0 / 0.4);
    transform: translate(-50%, -50%);
  }
  .portal-panel header {
    display: flex;
    align-items: start;
    justify-content: space-between;
  }
  .portal-panel header > span {
    display: grid;
  }
  .portal-panel header small {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
    letter-spacing: 0.12em;
  }
  .portal-panel h2 {
    margin-top: 5px;
    font-size: 1.8rem;
    font-weight: 520;
    letter-spacing: -0.04em;
  }
  .portal-primary {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    margin-top: 24px;
  }
  .portal-primary button {
    display: grid;
    grid-template-columns: 26px 1fr;
    gap: 10px;
    min-height: 104px;
    align-content: end;
    padding: 14px;
    border-radius: 17px;
    background: color-mix(in oklab, var(--proto-ink) 5%, transparent);
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .portal-primary button:hover {
    background: var(--proto-accent-soft);
    color: var(--proto-accent);
  }
  .portal-primary button span {
    display: grid;
  }
  .portal-primary button strong {
    font-size: 0.8rem;
  }
  .portal-primary button small {
    margin-top: 3px;
    color: var(--proto-muted);
    font-size: 0.62rem;
  }
  .portal-secondary {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }
  .portal-secondary button {
    display: inline-flex;
    min-height: 36px;
    align-items: center;
    gap: 7px;
    padding: 0 12px;
    border-radius: 11px;
    background: transparent;
    color: var(--proto-muted);
    opacity: 0.55;
  }
  .portal-panel footer {
    margin-top: 22px;
    padding-top: 14px;
    border-top: 1px solid var(--proto-line);
    color: var(--proto-muted);
    font-size: 0.64rem;
  }

  @keyframes spin-record {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 1000px) {
    .b-header {
      grid-template-columns: 1fr auto;
    }
    .b-local-nav {
      display: none;
    }
    .b-realm {
      grid-template-columns: 0.68fr 1.32fr;
      padding: 36px 26px;
    }
    .b-realm-ribbon {
      grid-template-columns: 1.25fr 0.75fr;
      height: 460px;
    }
    .b-realm-ribbon > button:nth-child(3),
    .b-realm-ribbon > button:nth-child(4) {
      display: none;
    }
    .b-novel {
      grid-template-columns: 0.55fr 0.82fr 1.15fr;
      gap: 24px;
      padding: 34px 24px;
    }
    .b-book-stack button:last-child {
      display: none;
    }
    .b-book-stack {
      grid-template-columns: repeat(2, 1fr);
    }
    .b-music {
      grid-template-columns: 0.8fr 0.72fr;
      padding: 34px 26px;
    }
    .b-track-list {
      grid-column: 1/-1;
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      gap: 0 28px;
    }
  }

  @media (max-width: 700px) {
    .b-shell {
      grid-template-rows: 58px minmax(0, 1fr) auto auto;
    }
    .b-header {
      padding: 6px 9px;
    }
    .portal-trigger {
      grid-template-columns: 31px auto 16px;
      min-height: 46px;
      border-radius: 15px;
    }
    .portal-trigger .frame-mark {
      width: 28px;
      height: 28px;
    }
    .b-header-actions .window-dots,
    .audio-restore {
      display: none;
    }
    .b-mobile-nav {
      display: grid;
      grid-template-columns: repeat(4, 1fr);
      min-height: calc(60px + env(safe-area-inset-bottom, 0px));
      padding: 4px 8px env(safe-area-inset-bottom, 0px);
    }
    .b-mobile-nav button {
      display: grid;
      place-items: center;
      align-content: center;
      gap: 3px;
      border-radius: 12px;
      background: transparent;
      color: var(--proto-muted);
    }
    .b-mobile-nav button.active {
      color: var(--proto-accent);
    }
    .b-mobile-nav span {
      font-size: 0.58rem;
    }
    .b-realm {
      display: block;
      padding: 28px 16px 48px;
    }
    .b-realm-intro h1 {
      font-size: clamp(3.1rem, 14vw, 4.2rem);
    }
    .b-realm-intro p {
      margin-top: 14px;
    }
    .b-realm-ribbon {
      grid-template-columns: 1.2fr 0.8fr;
      height: 300px;
      margin-top: 28px;
    }
    .b-realm-ribbon > button.lifted {
      transform: translateY(-10px);
    }
    .b-realm-facts {
      margin-top: 20px;
      overflow: auto;
      white-space: nowrap;
    }
    .b-novel {
      display: grid;
      grid-template-columns: 1fr 1fr;
      align-items: start;
      padding: 26px 16px 46px;
    }
    .b-novel-heading {
      grid-column: 1/-1;
    }
    .b-novel-heading h1 {
      font-size: 3.6rem;
    }
    .b-featured-book {
      border-radius: 20px;
    }
    .b-book-stack {
      grid-template-columns: 1fr;
    }
    .b-book-stack button:nth-child(2),
    .b-book-stack button:last-child {
      display: none;
    }
    .b-music {
      display: grid;
      grid-template-columns: 1fr 1fr;
      align-items: center;
      padding: 24px 16px 48px;
    }
    .b-record-stage {
      grid-column: 1/-1;
      width: min(72vw, 310px);
      margin: auto;
    }
    .b-album-copy h1 {
      font-size: 3.7rem;
    }
    .b-track-list {
      grid-column: 1/-1;
      display: block;
    }
    .b-track-list li:nth-child(n + 4) {
      display: none;
    }
    .b-reader > header {
      grid-template-columns: auto 1fr auto;
      height: 56px;
      padding-inline: 8px;
    }
    .portal-trigger.compact > span:nth-child(2),
    .portal-trigger.compact > .compact-chevron {
      display: none;
    }
    .portal-trigger.compact {
      grid-template-columns: 28px;
      padding-inline: 7px;
    }
    .b-reader-location span {
      display: none;
    }
    .b-reader-location strong {
      overflow: hidden;
      max-width: 55vw;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .b-reader main {
      height: calc(100% - 56px);
      padding: 48px 22px 130px;
    }
    .reader-back,
    .b-reader-margin {
      display: none;
    }
    .b-reader article h1 {
      margin-bottom: 38px;
      font-size: 2.2rem;
    }
    .b-reader article p {
      font-size: 1rem;
      line-height: 1.82;
    }
    .b-reader-audio,
    .b-reader-restore {
      right: 50%;
      bottom: 82px;
      transform: translateX(50%);
    }
    .portal-panel {
      top: auto;
      right: 8px;
      bottom: 8px;
      left: 8px;
      width: auto;
      transform: none;
    }
    .portal-primary {
      grid-template-columns: 1fr;
    }
    .portal-primary button {
      min-height: 68px;
      align-content: center;
    }
    .portal-secondary {
      overflow: auto;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .portal-trigger,
    .portal-backdrop {
      backdrop-filter: none;
    }
    .portal-trigger {
      background: var(--proto-surface-strong);
    }
  }
</style>
