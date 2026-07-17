<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import BookOpen from '@lucide/svelte/icons/book-open';
  import Compass from '@lucide/svelte/icons/compass';
  import Disc3 from '@lucide/svelte/icons/disc-3';
  import LibraryBig from '@lucide/svelte/icons/library-big';
  import Moon from '@lucide/svelte/icons/moon';
  import Play from '@lucide/svelte/icons/play';
  import Radio from '@lucide/svelte/icons/radio';
  import Search from '@lucide/svelte/icons/search';
  import Sun from '@lucide/svelte/icons/sun';
  import Workflow from '@lucide/svelte/icons/workflow';
  import { prefersReducedMotion } from 'svelte/motion';
  import { fly } from 'svelte/transition';
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
    type PrototypeSpace,
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

  const activeSpace = $derived(spaceOptions.find((item) => item.id === space) ?? spaceOptions[0]);
  const playing = $derived(audioState === 'playing');

  const iconForSpace = (id: PrototypeSpace) => {
    if (id === 'novel' || id === 'reader') return BookOpen;
    if (id === 'music') return Disc3;
    return Compass;
  };
</script>

<div class="variant-a" class:is-reader={space === 'reader'}>
  {#if space === 'reader'}
    <section class="a-reader-shell">
      <header class="a-reader-topbar">
        <button
          type="button"
          class="icon-button"
          onclick={() => onspacechange('novel')}
          aria-label="返回小说空间"
        >
          <ArrowLeft size={18} strokeWidth={1.5} aria-hidden="true" />
        </button>
        <div class="reader-book-title">
          <span>在潮汐线醒来</span>
          <strong>第 18 章 · 风暴将至</strong>
        </div>
        <div class="reader-topbar-actions">
          {#if audioState === 'none'}
            <button class="restore-audio" type="button" onclick={() => onaudiochange('paused')}
              >恢复环境音频入口</button
            >
          {/if}
          <button
            type="button"
            class="icon-button"
            onclick={() => onthemechange(theme === 'dark' ? 'light' : 'dark')}
            aria-label="切换明暗主题"
          >
            {#if theme === 'dark'}
              <Sun size={17} strokeWidth={1.5} aria-hidden="true" />
            {:else}
              <Moon size={17} strokeWidth={1.5} aria-hidden="true" />
            {/if}
          </button>
        </div>
      </header>

      <main class="a-reader-canvas">
        <article>
          <span class="chapter-index">十八</span>
          <h1>风暴将至</h1>
          {#each readerParagraphs as paragraph, index (`a-${index}`)}
            <p>{paragraph}</p>
          {/each}
        </article>
        <span class="reader-progress" aria-label="阅读进度 62%">62%</span>
        {#if audioState !== 'none'}
          <div class="reader-audio-anchor">
            <ContinuousMediaBar
              state={audioState}
              tone="A"
              reader
              onstatechange={onaudiochange}
              {onqueueopen}
            />
          </div>
        {/if}
      </main>
    </section>
  {:else}
    <aside class="a-global-rail">
      <button
        class="a-brand"
        type="button"
        onclick={() => onspacechange('realm')}
        aria-label="LanJing 境场"
      >
        <span class="frame-mark" aria-hidden="true"><i></i><i></i></span>
        <span><strong>览境</strong><small>LanJing</small></span>
      </button>

      <nav class="a-global-nav" aria-label="全局导航">
        <button
          type="button"
          class:active={space === 'realm'}
          onclick={() => onspacechange('realm')}
        >
          <Compass size={19} strokeWidth={1.45} aria-hidden="true" />
          <span>境场</span>
        </button>
        <button type="button" disabled title="本原型未覆盖资料库">
          <LibraryBig size={19} strokeWidth={1.45} aria-hidden="true" />
          <span>资料库</span>
        </button>
        <button type="button" disabled title="本原型未覆盖来源管理">
          <Radio size={19} strokeWidth={1.45} aria-hidden="true" />
          <span>来源</span>
        </button>
      </nav>

      <button class="a-studio" type="button" disabled title="规则工作室属于独立工具上下文">
        <Workflow size={18} strokeWidth={1.45} aria-hidden="true" />
        <span>规则工作室</span>
      </button>
    </aside>

    <section class="a-frame">
      <header class="a-window-context">
        <div class="a-window-title">
          <span class="frame-mark small" aria-hidden="true"><i></i><i></i></span>
          <span>LanJing</span>
          <i></i>
          <strong>{activeSpace.label}空间</strong>
        </div>
        <div class="a-window-actions">
          {#if audioState === 'none'}
            <button class="restore-audio" type="button" onclick={() => onaudiochange('paused')}
              >恢复演示音频</button
            >
          {/if}
          <button type="button" class="search-button" aria-label="打开全局内容搜索">
            <Search size={16} strokeWidth={1.45} aria-hidden="true" />
            <span>搜索内容</span>
            <kbd>⌘ K</kbd>
          </button>
          <button
            type="button"
            class="icon-button"
            onclick={() => onthemechange(theme === 'dark' ? 'light' : 'dark')}
            aria-label="切换明暗主题"
          >
            {#if theme === 'dark'}
              <Sun size={17} strokeWidth={1.5} aria-hidden="true" />
            {:else}
              <Moon size={17} strokeWidth={1.5} aria-hidden="true" />
            {/if}
          </button>
          <div class="window-controls" aria-hidden="true"><i></i><i></i><i></i></div>
        </div>
      </header>

      <nav class="a-media-tabs" aria-label="媒体空间">
        {#each spaceOptions.filter((item) => item.id !== 'reader') as option (option.id)}
          {@const Icon = iconForSpace(option.id)}
          <button
            type="button"
            class:active={space === option.id}
            onclick={() => onspacechange(option.id)}
            aria-current={space === option.id ? 'page' : undefined}
          >
            <Icon size={16} strokeWidth={1.45} aria-hidden="true" />
            <span>{option.label}</span>
          </button>
        {/each}
        <span class="a-session-state">
          <i class:playing></i>{playing
            ? '音频会话连续'
            : audioState === 'paused'
              ? '音频已暂停'
              : '无音频会话'}
        </span>
      </nav>

      <main class="a-content">
        {#key space}
          <div
            class="a-space-scene"
            in:fly={{
              y: prefersReducedMotion.current ? 0 : 20,
              duration: prefersReducedMotion.current ? 0 : 420,
            }}
          >
            {#if space === 'realm'}
              <section class="a-realm">
                <div class="a-realm-copy">
                  <span class="scene-kicker">7 月 14 日 · 傍晚</span>
                  <h1>从这里，进入<br />今天的内容。</h1>
                  <p>进度、收藏与最近活动组成确定性境场。来源默认聚合，不伪造推荐原因。</p>
                  <button
                    type="button"
                    class="primary-action"
                    onclick={() => onspacechange('novel')}
                  >
                    继续阅读
                    <span><ArrowLeft size={15} strokeWidth={1.5} aria-hidden="true" /></span>
                  </button>
                </div>

                <div class="a-realm-mosaic">
                  {#each realmStories as story (story.id)}
                    <button
                      type="button"
                      class={[`story-${story.format}`, story.id === 'realm-dawn' && 'featured']}
                      onclick={() =>
                        onspacechange(
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
                        kicker={story.kicker}
                        art={story.art}
                        motif={story.motif}
                        foreground={story.foreground}
                        ratio={story.format === 'wide'
                          ? 'landscape'
                          : story.format === 'square'
                            ? 'square'
                            : 'portrait'}
                      />
                      <span class="story-meta"
                        ><strong>{story.kind} · {story.title}</strong><small>{story.note}</small
                        ></span
                      >
                    </button>
                  {/each}
                </div>
              </section>
            {:else if space === 'novel'}
              <section class="a-novel">
                <header class="section-heading">
                  <div>
                    <span class="scene-kicker">小说空间</span>
                    <h1>活书架</h1>
                    <p>继续阅读与最近活动共用一座书架。</p>
                  </div>
                  <div class="segmented" aria-label="小说空间页签">
                    <button class="active" type="button">活书架</button>
                    <button type="button">发现</button>
                  </div>
                </header>
                <div class="a-book-grid">
                  {#each novelShelf as book, index (book.id)}
                    <button
                      type="button"
                      class="book-card"
                      onclick={() => index === 0 && onspacechange('reader')}
                    >
                      <CoverArt
                        title={book.title}
                        creator={book.creator}
                        kicker={book.kicker}
                        art={book.art}
                        motif={book.motif}
                        foreground={book.foreground}
                      />
                      <span class="book-copy">
                        <strong>{book.title}</strong>
                        <small>{book.creator} · {book.status}</small>
                        <span>{book.chapter}</span>
                        <i><b style:--book-progress={book.progress / 100}></b></i>
                      </span>
                    </button>
                  {/each}
                </div>
              </section>
            {:else}
              <section class="a-music">
                <div class="music-art-stage">
                  <CoverArt
                    title={albums[0].title}
                    creator={albums[0].creator}
                    kicker={albums[0].kicker}
                    art={albums[0].art}
                    motif={albums[0].motif}
                    foreground={albums[0].foreground}
                    ratio="square"
                  />
                </div>
                <div class="music-detail">
                  <span class="scene-kicker">音乐空间 · 专辑</span>
                  <h1>{albums[0].title}</h1>
                  <p>{albums[0].creator} · {albums[0].year} · {albums[0].mood}</p>
                  <button
                    type="button"
                    class="primary-action"
                    onclick={() => onaudiochange('paused')}
                  >
                    <Play size={16} fill="currentColor" strokeWidth={1.3} aria-hidden="true" />
                    播放专辑
                  </button>
                  <ol class="track-list">
                    {#each tracks as track, index (track.id)}
                      <li class:current={index === 0}>
                        <span>{String(index + 1).padStart(2, '0')}</span>
                        <p><strong>{track.title}</strong><small>{track.artist}</small></p>
                        <time>{track.duration}</time>
                      </li>
                    {/each}
                  </ol>
                </div>
              </section>
            {/if}
          </div>
        {/key}
      </main>

      <ContinuousMediaBar state={audioState} tone="A" onstatechange={onaudiochange} {onqueueopen} />

      <nav class="a-mobile-nav" aria-label="移动端全局与媒体导航">
        <button
          type="button"
          class:active={space === 'realm'}
          onclick={() => onspacechange('realm')}
        >
          <Compass size={18} strokeWidth={1.45} aria-hidden="true" /><span>境场</span>
        </button>
        <button
          type="button"
          class:active={space === 'novel'}
          onclick={() => onspacechange('novel')}
        >
          <BookOpen size={18} strokeWidth={1.45} aria-hidden="true" /><span>小说</span>
        </button>
        <button
          type="button"
          class:active={space === 'music'}
          onclick={() => onspacechange('music')}
        >
          <Disc3 size={18} strokeWidth={1.45} aria-hidden="true" /><span>音乐</span>
        </button>
        <button type="button" disabled>
          <LibraryBig size={18} strokeWidth={1.45} aria-hidden="true" /><span>资料库</span>
        </button>
      </nav>
    </section>
  {/if}
</div>

<style>
  :global(button) {
    -webkit-tap-highlight-color: transparent;
  }

  button {
    border: 0;
    font: inherit;
  }

  .variant-a {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr);
    min-height: 0;
    height: 100%;
    overflow: hidden;
    background: var(--proto-canvas);
    color: var(--proto-ink);
  }

  .variant-a.is-reader {
    display: block;
  }

  .a-global-rail {
    position: relative;
    z-index: 20;
    display: flex;
    min-height: 0;
    flex-direction: column;
    align-items: center;
    padding: 12px 8px 16px;
    border-right: 1px solid var(--proto-line);
    background: var(--proto-surface);
  }

  .a-brand {
    display: grid;
    width: 64px;
    justify-items: center;
    gap: 6px;
    padding: 8px 4px 10px;
    border-radius: 15px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .a-brand > span:last-child {
    display: grid;
    justify-items: center;
  }

  .a-brand strong {
    font-size: 0.72rem;
    font-weight: 600;
  }

  .a-brand small {
    color: var(--proto-muted);
    font-size: 0.52rem;
    letter-spacing: 0.05em;
  }

  .frame-mark {
    position: relative;
    display: block;
    width: 30px;
    height: 30px;
    color: var(--proto-accent);
  }

  .frame-mark i {
    position: absolute;
    width: 19px;
    height: 23px;
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

  .frame-mark.small {
    width: 22px;
    height: 22px;
  }

  .frame-mark.small i {
    width: 14px;
    height: 17px;
  }

  .a-global-nav {
    display: grid;
    width: 100%;
    gap: 6px;
    margin-top: 30px;
  }

  .a-global-nav button,
  .a-studio {
    display: grid;
    min-height: 56px;
    place-items: center;
    gap: 4px;
    border-radius: 13px;
    background: transparent;
    color: var(--proto-muted);
    cursor: pointer;
    transition:
      color 200ms cubic-bezier(0.32, 0.72, 0, 1),
      background 200ms cubic-bezier(0.32, 0.72, 0, 1),
      transform 200ms cubic-bezier(0.32, 0.72, 0, 1);
  }

  .a-global-nav button span,
  .a-studio span {
    font-size: 0.62rem;
  }

  .a-global-nav button.active {
    background: var(--proto-accent-soft);
    color: var(--proto-accent);
  }

  .a-global-nav button:not(:disabled):hover,
  .a-global-nav button:not(:disabled):focus-visible {
    background: color-mix(in oklab, var(--proto-ink) 6%, transparent);
    color: var(--proto-ink);
  }

  .a-global-nav button:active {
    transform: scale(0.96);
  }

  .a-global-nav button:disabled,
  .a-studio:disabled {
    cursor: default;
    opacity: 0.52;
  }

  .a-studio {
    width: 100%;
    margin-top: auto;
  }

  .a-frame {
    display: grid;
    grid-template-rows: 44px 52px minmax(0, 1fr) auto auto;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .a-window-context {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px 0 18px;
    border-bottom: 1px solid var(--proto-line);
    background: var(--proto-surface);
  }

  .a-window-title,
  .a-window-actions,
  .window-controls {
    display: flex;
    align-items: center;
  }

  .a-window-title {
    min-width: 0;
    gap: 8px;
    font-size: 0.72rem;
  }

  .a-window-title > span:not(.frame-mark) {
    font-weight: 600;
  }

  .a-window-title > i {
    width: 1px;
    height: 14px;
    background: var(--proto-line);
  }

  .a-window-title strong {
    overflow: hidden;
    color: var(--proto-muted);
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .a-window-actions {
    gap: 8px;
  }

  .icon-button,
  .search-button,
  .restore-audio {
    display: inline-flex;
    min-height: 32px;
    align-items: center;
    justify-content: center;
    border-radius: 10px;
    background: color-mix(in oklab, var(--proto-ink) 5%, transparent);
    color: var(--proto-muted);
    cursor: pointer;
    transition:
      color 180ms cubic-bezier(0.32, 0.72, 0, 1),
      background 180ms cubic-bezier(0.32, 0.72, 0, 1),
      transform 180ms cubic-bezier(0.32, 0.72, 0, 1);
  }

  .icon-button {
    width: 32px;
  }

  .search-button {
    gap: 8px;
    padding: 0 9px;
  }

  .search-button span,
  .restore-audio {
    font-size: 0.68rem;
  }

  .search-button kbd {
    color: var(--proto-muted);
    font-family: var(--font-code, monospace);
    font-size: 0.56rem;
  }

  .restore-audio {
    padding: 0 10px;
    color: var(--proto-accent);
  }

  .icon-button:hover,
  .icon-button:focus-visible,
  .search-button:hover,
  .search-button:focus-visible,
  .restore-audio:hover,
  .restore-audio:focus-visible {
    background: color-mix(in oklab, var(--proto-ink) 9%, transparent);
    color: var(--proto-ink);
  }

  .icon-button:active,
  .search-button:active,
  .restore-audio:active {
    transform: scale(0.95);
  }

  .window-controls {
    gap: 7px;
    padding-inline: 7px;
  }

  .window-controls i {
    width: 8px;
    border-radius: 50%;
    background: color-mix(in oklab, var(--proto-ink) 24%, transparent);
    aspect-ratio: 1;
  }

  .window-controls i:last-child {
    background: var(--proto-accent);
  }

  .a-media-tabs {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 4px;
    padding: 7px 18px;
    border-bottom: 1px solid var(--proto-line);
    background: color-mix(in oklab, var(--proto-surface) 82%, transparent);
  }

  .a-media-tabs button {
    display: inline-flex;
    min-height: 36px;
    align-items: center;
    gap: 7px;
    padding: 0 13px;
    border-radius: 11px;
    background: transparent;
    color: var(--proto-muted);
    cursor: pointer;
    transition:
      color 200ms cubic-bezier(0.32, 0.72, 0, 1),
      background 200ms cubic-bezier(0.32, 0.72, 0, 1),
      transform 200ms cubic-bezier(0.32, 0.72, 0, 1);
  }

  .a-media-tabs button.active {
    background: var(--proto-surface-strong);
    color: var(--proto-ink);
    box-shadow: 0 1px 0 rgb(255 255 255 / 0.06) inset;
  }

  .a-media-tabs button:active {
    transform: scale(0.96);
  }

  .a-session-state {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-left: auto;
    color: var(--proto-muted);
    font-size: 0.62rem;
    white-space: nowrap;
  }

  .a-session-state i {
    width: 6px;
    border-radius: 50%;
    background: var(--proto-muted);
    aspect-ratio: 1;
  }

  .a-session-state i.playing {
    background: var(--proto-accent);
    box-shadow: 0 0 0 4px var(--proto-accent-soft);
  }

  .a-content {
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }

  .a-space-scene {
    min-height: 100%;
  }

  .a-realm {
    display: grid;
    grid-template-columns: minmax(250px, 0.72fr) minmax(480px, 1.5fr);
    gap: clamp(32px, 5vw, 76px);
    align-items: center;
    width: min(1260px, 100%);
    min-height: 100%;
    margin-inline: auto;
    padding: clamp(34px, 5vw, 72px);
  }

  .a-realm-copy {
    align-self: center;
  }

  .scene-kicker {
    display: block;
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.64rem;
    letter-spacing: 0.1em;
  }

  h1,
  p {
    margin: 0;
  }

  .a-realm-copy h1 {
    margin-top: 14px;
    font-size: clamp(2.5rem, 5vw, 5.6rem);
    font-weight: 520;
    line-height: 0.94;
    letter-spacing: -0.06em;
  }

  .a-realm-copy p {
    max-width: 34ch;
    margin-top: 22px;
    color: var(--proto-muted);
    font-size: 0.86rem;
    line-height: 1.72;
  }

  .primary-action {
    display: inline-flex;
    min-height: 44px;
    align-items: center;
    gap: 10px;
    margin-top: 26px;
    padding: 0 8px 0 17px;
    border-radius: 999px;
    background: var(--proto-accent);
    color: var(--proto-on-accent);
    cursor: pointer;
    transition:
      transform 240ms cubic-bezier(0.32, 0.72, 0, 1),
      filter 240ms cubic-bezier(0.32, 0.72, 0, 1);
  }

  .primary-action > span {
    display: grid;
    width: 30px;
    place-items: center;
    border-radius: 50%;
    background: rgb(255 255 255 / 0.15);
    transform: rotate(180deg);
    aspect-ratio: 1;
  }

  .primary-action:hover,
  .primary-action:focus-visible {
    filter: brightness(1.08);
    transform: translateY(-2px);
  }

  .primary-action:active {
    transform: translateY(0) scale(0.98);
  }

  .a-realm-mosaic {
    display: grid;
    grid-template-columns: 1.35fr 0.78fr 0.88fr;
    grid-template-rows: minmax(150px, 1fr) minmax(150px, 0.9fr);
    gap: 12px;
    min-width: 0;
    height: min(560px, 62vh);
  }

  .a-realm-mosaic > button {
    position: relative;
    min-width: 0;
    overflow: hidden;
    padding: 0;
    border-radius: 18px;
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition:
      transform 420ms cubic-bezier(0.2, 0.9, 0.1, 1),
      filter 420ms cubic-bezier(0.2, 0.9, 0.1, 1);
  }

  .a-realm-mosaic > button:first-child {
    grid-row: 1 / -1;
  }

  .a-realm-mosaic > button:last-child {
    grid-column: 2 / -1;
  }

  .a-realm-mosaic > button:hover,
  .a-realm-mosaic > button:focus-visible {
    z-index: 2;
    filter: saturate(1.08);
    transform: translateY(-4px) scale(1.01);
  }

  .a-realm-mosaic :global(.cover-art) {
    width: 100%;
    height: 100%;
    aspect-ratio: auto;
  }

  .story-meta {
    position: absolute;
    right: 10px;
    bottom: 10px;
    left: 10px;
    display: none;
  }

  .a-novel {
    width: min(1180px, 100%);
    min-height: 100%;
    margin-inline: auto;
    padding: clamp(28px, 4vw, 56px);
  }

  .section-heading {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 20px;
  }

  .section-heading h1,
  .music-detail h1 {
    margin-top: 8px;
    font-size: clamp(2.4rem, 4vw, 4.6rem);
    font-weight: 520;
    line-height: 0.98;
    letter-spacing: -0.055em;
  }

  .section-heading p,
  .music-detail > p {
    margin-top: 10px;
    color: var(--proto-muted);
    font-size: 0.8rem;
  }

  .segmented {
    display: flex;
    gap: 3px;
    padding: 3px;
    border-radius: 12px;
    background: color-mix(in oklab, var(--proto-ink) 6%, transparent);
  }

  .segmented button {
    min-height: 34px;
    padding: 0 14px;
    border-radius: 9px;
    background: transparent;
    color: var(--proto-muted);
    cursor: pointer;
  }

  .segmented button.active {
    background: var(--proto-surface-strong);
    color: var(--proto-ink);
  }

  .a-book-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: clamp(14px, 2vw, 24px);
    margin-top: clamp(28px, 4vw, 48px);
  }

  .book-card {
    min-width: 0;
    padding: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .book-card :global(.cover-art) {
    transition:
      transform 420ms cubic-bezier(0.2, 0.9, 0.1, 1),
      filter 420ms cubic-bezier(0.2, 0.9, 0.1, 1);
  }

  .book-card:hover :global(.cover-art),
  .book-card:focus-visible :global(.cover-art) {
    filter: saturate(1.05);
    transform: translateY(-5px);
  }

  .book-copy {
    display: grid;
    margin-top: 13px;
  }

  .book-copy > strong {
    font-size: 0.86rem;
    font-weight: 600;
  }

  .book-copy > small,
  .book-copy > span {
    overflow: hidden;
    margin-top: 3px;
    color: var(--proto-muted);
    font-size: 0.66rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .book-copy > i {
    position: relative;
    height: 2px;
    overflow: hidden;
    margin-top: 10px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--proto-ink) 10%, transparent);
  }

  .book-copy > i b {
    position: absolute;
    inset: 0;
    background: var(--proto-accent);
    transform: scaleX(var(--book-progress));
    transform-origin: left center;
  }

  .a-music {
    display: grid;
    grid-template-columns: minmax(270px, 0.8fr) minmax(380px, 1.2fr);
    gap: clamp(38px, 7vw, 96px);
    align-items: center;
    width: min(1120px, 100%);
    min-height: 100%;
    margin-inline: auto;
    padding: clamp(30px, 5vw, 70px);
  }

  .music-art-stage {
    width: min(460px, 100%);
    padding: clamp(8px, 1.3vw, 16px);
    border: 1px solid var(--proto-line);
    border-radius: 28px;
    background: color-mix(in oklab, var(--proto-surface-strong) 72%, transparent);
    box-shadow: 0 28px 80px rgb(8 10 13 / 0.14);
  }

  .music-detail .primary-action {
    padding-right: 18px;
  }

  .track-list {
    margin: 28px 0 0;
    padding: 0;
    list-style: none;
  }

  .track-list li {
    display: grid;
    grid-template-columns: 26px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    min-height: 50px;
    border-bottom: 1px solid var(--proto-line);
    color: var(--proto-muted);
  }

  .track-list li.current {
    color: var(--proto-accent);
  }

  .track-list li > span,
  .track-list time {
    font-family: var(--font-code, monospace);
    font-size: 0.6rem;
  }

  .track-list p {
    display: grid;
  }

  .track-list strong {
    color: var(--proto-ink);
    font-size: 0.76rem;
    font-weight: 560;
  }

  .track-list small {
    margin-top: 2px;
    font-size: 0.62rem;
  }

  .a-mobile-nav {
    display: none;
  }

  .a-reader-shell {
    display: grid;
    grid-template-rows: 48px minmax(0, 1fr);
    height: 100%;
    background: var(--reader-surface);
    color: var(--reader-ink);
  }

  .a-reader-topbar {
    position: relative;
    z-index: 4;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    padding: 0 14px;
    border-bottom: 1px solid color-mix(in oklab, var(--reader-ink) 10%, transparent);
    background: color-mix(in oklab, var(--reader-surface) 92%, transparent);
  }

  .a-reader-topbar > .icon-button {
    justify-self: start;
  }

  .reader-book-title {
    display: grid;
    justify-items: center;
    font-size: 0.62rem;
  }

  .reader-book-title span {
    color: color-mix(in oklab, var(--reader-ink) 60%, transparent);
  }

  .reader-book-title strong {
    font-weight: 560;
  }

  .reader-topbar-actions {
    display: flex;
    justify-self: end;
    gap: 8px;
  }

  .a-reader-canvas {
    position: relative;
    min-height: 0;
    overflow: auto;
    padding: clamp(58px, 8vh, 110px) 24px 150px;
  }

  .a-reader-canvas article {
    width: min(680px, 100%);
    margin-inline: auto;
    font-family: var(--font-reader, serif);
  }

  .chapter-index {
    display: block;
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.68rem;
    text-align: center;
  }

  .a-reader-canvas h1 {
    margin: 13px 0 48px;
    font-family: var(--font-reader, serif);
    font-size: clamp(2rem, 4vw, 3.1rem);
    font-weight: 600;
    letter-spacing: -0.03em;
    text-align: center;
  }

  .a-reader-canvas p {
    margin: 0 0 1.2em;
    font-size: clamp(1.03rem, 1.5vw, 1.18rem);
    line-height: 1.88;
  }

  .reader-progress {
    position: fixed;
    top: 50%;
    right: 18px;
    color: color-mix(in oklab, var(--reader-ink) 44%, transparent);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
    transform: rotate(90deg) translateY(-50%);
  }

  .reader-audio-anchor {
    position: fixed;
    right: 50%;
    bottom: 28px;
    z-index: 8;
    transform: translateX(50%);
  }

  @media (max-width: 1120px) {
    .variant-a {
      grid-template-columns: 72px minmax(0, 1fr);
    }

    .a-global-rail {
      padding-inline: 5px;
    }

    .a-brand {
      width: 58px;
    }

    .a-global-nav button span,
    .a-studio span,
    .a-brand > span:last-child {
      display: none;
    }

    .a-realm {
      grid-template-columns: minmax(210px, 0.7fr) minmax(430px, 1.3fr);
      gap: 30px;
      padding: 34px;
    }

    .a-realm-mosaic {
      height: min(500px, 60vh);
    }
  }

  @media (max-width: 900px) {
    .variant-a {
      display: block;
    }

    .a-global-rail {
      display: none;
    }

    .a-frame {
      height: 100%;
      grid-template-rows: 44px 48px minmax(0, 1fr) auto auto;
    }

    .a-mobile-nav {
      display: grid;
      grid-template-columns: repeat(4, 1fr);
      min-height: calc(62px + env(safe-area-inset-bottom, 0px));
      padding: 5px 8px env(safe-area-inset-bottom, 0px);
      border-top: 1px solid var(--proto-line);
      background: var(--proto-surface);
    }

    .a-mobile-nav button {
      display: grid;
      min-width: 0;
      place-items: center;
      align-content: center;
      gap: 3px;
      border-radius: 12px;
      background: transparent;
      color: var(--proto-muted);
    }

    .a-mobile-nav button span {
      font-size: 0.6rem;
    }

    .a-mobile-nav button.active {
      color: var(--proto-accent);
    }

    .a-mobile-nav button:disabled {
      opacity: 0.48;
    }

    .a-realm {
      grid-template-columns: 0.72fr 1.28fr;
      min-height: auto;
      padding: 30px 24px 46px;
    }

    .a-realm-mosaic {
      grid-template-columns: 1.2fr 0.8fr;
      grid-template-rows: repeat(2, minmax(130px, 1fr));
      height: 430px;
    }

    .a-realm-mosaic > button:first-child {
      grid-row: 1 / -1;
    }

    .a-realm-mosaic > button:nth-child(3) {
      display: none;
    }

    .a-realm-mosaic > button:last-child {
      grid-column: auto;
    }

    .a-book-grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .book-card:last-child {
      display: none;
    }

    .a-music {
      grid-template-columns: minmax(230px, 0.82fr) minmax(320px, 1.18fr);
      gap: 36px;
      padding: 32px 28px;
    }
  }

  @media (max-width: 640px) {
    .a-window-context {
      padding-inline: 10px;
    }

    .a-window-title .frame-mark,
    .a-window-title > span:not(.frame-mark),
    .a-window-title > i,
    .search-button,
    .window-controls,
    .a-session-state {
      display: none;
    }

    .a-window-title strong {
      color: var(--proto-ink);
      font-size: 0.78rem;
    }

    .restore-audio {
      max-width: 126px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .a-media-tabs {
      gap: 2px;
      padding-inline: 8px;
    }

    .a-media-tabs button {
      flex: 1;
      justify-content: center;
      padding-inline: 7px;
    }

    .a-realm {
      display: block;
      padding: 26px 16px 48px;
    }

    .a-realm-copy h1 {
      font-size: clamp(2.6rem, 13vw, 3.8rem);
    }

    .a-realm-copy p {
      max-width: 40ch;
      margin-top: 16px;
      font-size: 0.78rem;
    }

    .a-realm-copy .primary-action {
      margin-top: 18px;
    }

    .a-realm-mosaic {
      grid-template-columns: 1.2fr 0.8fr;
      height: 310px;
      margin-top: 28px;
    }

    .a-realm-mosaic > button:last-child {
      display: none;
    }

    .a-novel {
      padding: 26px 16px 46px;
    }

    .section-heading {
      align-items: start;
    }

    .section-heading h1 {
      font-size: 2.6rem;
    }

    .section-heading p {
      max-width: 24ch;
    }

    .segmented {
      flex-direction: column;
    }

    .segmented button {
      min-height: 30px;
      padding-inline: 10px;
      font-size: 0.66rem;
    }

    .a-book-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 18px 12px;
      margin-top: 28px;
    }

    .book-card:last-child {
      display: block;
    }

    .a-music {
      display: block;
      padding: 24px 16px 48px;
    }

    .music-art-stage {
      width: min(72vw, 290px);
      margin-inline: auto;
      border-radius: 22px;
    }

    .music-detail {
      margin-top: 26px;
    }

    .music-detail h1 {
      font-size: 3rem;
    }

    .track-list {
      margin-top: 22px;
    }

    .a-reader-topbar {
      grid-template-columns: auto minmax(0, 1fr) auto;
      padding-inline: 8px;
    }

    .reader-book-title span {
      display: none;
    }

    .reader-book-title strong {
      overflow: hidden;
      max-width: 42vw;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .reader-topbar-actions .restore-audio {
      display: none;
    }

    .a-reader-canvas {
      padding: 48px 22px 130px;
    }

    .a-reader-canvas h1 {
      margin-bottom: 38px;
      font-size: 2.1rem;
    }

    .a-reader-canvas p {
      font-size: 1rem;
      line-height: 1.82;
    }

    .reader-progress {
      display: none;
    }

    .reader-audio-anchor {
      bottom: 82px;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .a-window-context,
    .a-media-tabs,
    .a-reader-topbar {
      background: var(--proto-surface);
    }
  }
</style>
