<script lang="ts">
  import {
    ArrowLeft,
    BookOpen,
    Command,
    Compass,
    Disc3,
    Moon,
    Play,
    Search,
    Sun,
    X,
  } from '@lucide/svelte';
  import { prefersReducedMotion } from 'svelte/motion';
  import { fade, fly, scale } from 'svelte/transition';
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

  let commandOpen = $state(false);

  function choose(next: PrototypeSpace): void {
    onspacechange(next);
    commandOpen = false;
  }

  function handleKeydown(event: KeyboardEvent): void {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      commandOpen = !commandOpen;
    }
    if (event.key === 'Escape') commandOpen = false;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="variant-c" class:reader={space === 'reader'}>
  <button class="c-mark" type="button" onclick={() => choose('realm')} aria-label="LanJing 境场">
    <span aria-hidden="true"><i></i><i></i></span><small>LJ</small>
  </button>

  <nav class="c-edge-nav" aria-label="边缘空间门户">
    {#each spaceOptions.filter((item) => item.id !== 'reader') as option (option.id)}
      <button
        type="button"
        class:active={space === option.id}
        onclick={() => choose(option.id)}
        aria-label={option.label}
      >
        <i></i><span>{option.label}</span>
      </button>
    {/each}
  </nav>

  <div class="c-top-actions">
    {#if audioState === 'none'}<button
        class="c-audio-restore"
        type="button"
        onclick={() => onaudiochange('paused')}>恢复音频</button
      >{/if}
    <button type="button" class="c-command-trigger" onclick={() => (commandOpen = true)}
      ><Search size={15} strokeWidth={1.5} aria-hidden="true" /><span>切换空间</span><kbd>⌘K</kbd
      ></button
    >
    <button
      type="button"
      class="c-theme"
      onclick={() => onthemechange(theme === 'dark' ? 'light' : 'dark')}
      aria-label="切换主题"
    >
      {#if theme === 'dark'}<Sun size={16} strokeWidth={1.5} aria-hidden="true" />{:else}<Moon
          size={16}
          strokeWidth={1.5}
          aria-hidden="true"
        />{/if}
    </button>
  </div>

  {#if space === 'reader'}
    <main class="c-reader-canvas">
      <button class="c-reader-back" type="button" onclick={() => onspacechange('novel')}
        ><ArrowLeft size={16} strokeWidth={1.5} />活书架</button
      >
      <article in:fade={{ duration: prefersReducedMotion.current ? 0 : 480 }}>
        <header>
          <span>在潮汐线醒来 · 018</span>
          <h1>风暴将至</h1>
        </header>
        {#each readerParagraphs as paragraph, index (`c-${index}`)}<p>{paragraph}</p>{/each}
      </article>
      <aside class="c-reading-meter"><b></b><span>62%</span></aside>
      {#if audioState !== 'none'}
        <div class="c-reader-audio">
          <ContinuousMediaBar
            state={audioState}
            tone="C"
            reader
            onstatechange={onaudiochange}
            {onqueueopen}
          />
        </div>
      {/if}
    </main>
  {:else}
    <main class="c-canvas">
      {#key space}
        <div
          class="c-scene"
          in:fly={{
            x: prefersReducedMotion.current ? 0 : 34,
            duration: prefersReducedMotion.current ? 0 : 540,
          }}
        >
          {#if space === 'realm'}
            <section class="c-realm">
              <div class="c-realm-art">
                <CoverArt
                  title={realmStories[0].title}
                  creator={realmStories[0].creator}
                  kicker="REALM 01"
                  art={realmStories[0].art}
                  motif={realmStories[0].motif}
                  foreground={realmStories[0].foreground}
                  ratio="landscape"
                  quiet
                />
                <div class="c-realm-overlay">
                  <span>确定性境场 · 最近活动</span>
                  <h1>在潮汐线<br />醒来</h1>
                  <button type="button" onclick={() => onspacechange('reader')}
                    >继续阅读 <i><ArrowLeft size={15} strokeWidth={1.5} /></i></button
                  >
                </div>
              </div>
              <ol class="c-realm-index">
                {#each realmStories.slice(1) as story, index (story.id)}
                  <li>
                    <button
                      type="button"
                      onclick={() => choose(story.kind === '音乐' ? 'music' : 'realm')}
                    >
                      <span>{String(index + 2).padStart(2, '0')}</span>
                      <p>
                        <strong>{story.title}</strong><small>{story.kind} · {story.note}</small>
                      </p>
                      <i style:--index-art={story.art}></i>
                    </button>
                  </li>
                {/each}
              </ol>
              <p class="c-realm-note">来源默认聚合 / 进度与收藏构成视野 / 无伪造个性化因果</p>
            </section>
          {:else if space === 'novel'}
            <section class="c-novel">
              <header>
                <span>N / 01</span>
                <h1>活书架</h1>
                <p>最近阅读改变顺序，不改变用户固定入口。</p>
              </header>
              <div class="c-shelf-line" aria-hidden="true"></div>
              <div class="c-horizontal-shelf">
                {#each novelShelf as book, index (book.id)}
                  <button
                    type="button"
                    class:primary={index === 0}
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
                    <span
                      ><small>{String(index + 1).padStart(2, '0')} / {book.progress}%</small><strong
                        >{book.title}</strong
                      ><i>{book.chapter}</i></span
                    >
                  </button>
                {/each}
              </div>
              <nav class="c-novel-tabs">
                <button class="active" type="button">活书架</button><button type="button"
                  >发现</button
                >
              </nav>
            </section>
          {:else}
            <section class="c-music">
              <div class="c-disc" class:playing={audioState === 'playing'}>
                <CoverArt
                  title={albums[0].title}
                  art={albums[0].art}
                  motif={albums[0].motif}
                  foreground={albums[0].foreground}
                  ratio="square"
                  quiet
                />
                <i aria-hidden="true"></i>
              </div>
              <div class="c-music-title">
                <span>M / {albums[0].year}</span>
                <h1>离岸<br />信号</h1>
                <p>{albums[0].creator} · {albums[0].mood}</p>
                <button type="button" onclick={() => onaudiochange('paused')}
                  ><Play size={16} fill="currentColor" strokeWidth={1.3} />播放专辑</button
                >
              </div>
              <ol class="c-music-queue">
                {#each tracks as track, index (track.id)}<li class:current={index === 0}>
                    <span>{index + 1}</span><strong>{track.title}</strong><time
                      >{track.duration}</time
                    >
                  </li>{/each}
              </ol>
            </section>
          {/if}
        </div>
      {/key}
    </main>

    <ContinuousMediaBar state={audioState} tone="C" onstatechange={onaudiochange} {onqueueopen} />
  {/if}

  <nav class="c-mobile-nav" aria-label="移动端边缘门户">
    <button type="button" class:active={space === 'realm'} onclick={() => choose('realm')}
      ><Compass size={18} strokeWidth={1.45} /><span>境场</span></button
    >
    <button
      type="button"
      class:active={space === 'novel' || space === 'reader'}
      onclick={() => choose('novel')}
      ><BookOpen size={18} strokeWidth={1.45} /><span>小说</span></button
    >
    <button type="button" class:active={space === 'music'} onclick={() => choose('music')}
      ><Disc3 size={18} strokeWidth={1.45} /><span>音乐</span></button
    >
    <button type="button" onclick={() => (commandOpen = true)}
      ><Command size={18} strokeWidth={1.45} /><span>命令</span></button
    >
  </nav>

  {#if commandOpen}
    <button
      class="command-backdrop"
      type="button"
      aria-label="关闭空间命令"
      onclick={() => (commandOpen = false)}
      transition:fade={{ duration: prefersReducedMotion.current ? 0 : 160 }}
    ></button>
    <div
      class="command-palette"
      role="dialog"
      aria-modal="true"
      aria-labelledby="command-title"
      transition:scale={{
        start: prefersReducedMotion.current ? 1 : 0.96,
        duration: prefersReducedMotion.current ? 0 : 280,
      }}
    >
      <header>
        <Search size={17} strokeWidth={1.5} />
        <div>
          <span>空间命令</span>
          <h2 id="command-title">去往…</h2>
        </div>
        <button type="button" onclick={() => (commandOpen = false)} aria-label="关闭"
          ><X size={17} strokeWidth={1.5} /></button
        >
      </header>
      <div class="command-list">
        {#each spaceOptions as option, index (option.id)}
          <button type="button" class:active={space === option.id} onclick={() => choose(option.id)}
            ><kbd>0{index + 1}</kbd><span
              ><strong>{option.label}</strong><small
                >{option.id === 'realm'
                  ? '跨媒体发现'
                  : option.id === 'novel'
                    ? '活书架与发现'
                    : option.id === 'music'
                      ? '专辑与队列'
                      : '沉浸阅读'}</small
              ></span
            ><i>↵</i></button
          >
        {/each}
      </div>
      <footer>全局搜索与命令面板保持分离。</footer>
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
  .variant-c {
    position: relative;
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    height: 100%;
    overflow: hidden;
    background: var(--proto-canvas);
    color: var(--proto-ink);
  }
  .variant-c.reader {
    display: block;
    background: var(--reader-surface);
    color: var(--reader-ink);
  }
  .c-mark {
    position: fixed;
    top: 18px;
    left: 18px;
    z-index: 20;
    display: grid;
    justify-items: center;
    gap: 2px;
    width: 46px;
    padding: 7px;
    border-radius: 14px;
    background: color-mix(in oklab, var(--proto-surface-strong) 78%, transparent);
    color: var(--proto-accent);
    cursor: pointer;
    backdrop-filter: blur(14px);
  }
  .c-mark > span {
    position: relative;
    width: 26px;
    height: 26px;
  }
  .c-mark i {
    position: absolute;
    width: 16px;
    height: 20px;
    border: 1.5px solid currentColor;
  }
  .c-mark i:first-child {
    top: 1px;
    left: 2px;
    border-right-color: transparent;
  }
  .c-mark i:last-child {
    right: 2px;
    bottom: 1px;
    border-left-color: transparent;
  }
  .c-mark small {
    font-family: var(--font-code, monospace);
    font-size: 0.5rem;
    letter-spacing: 0.1em;
  }
  .c-edge-nav {
    position: fixed;
    top: 50%;
    left: 20px;
    z-index: 18;
    display: grid;
    gap: 12px;
    transform: translateY(-50%);
  }
  .c-edge-nav button {
    display: grid;
    grid-template-columns: 8px 0fr;
    gap: 8px;
    align-items: center;
    width: 34px;
    min-height: 30px;
    overflow: hidden;
    padding: 0 10px;
    border-radius: 999px;
    background: transparent;
    color: var(--proto-muted);
    cursor: pointer;
    transition:
      grid-template-columns 320ms cubic-bezier(0.32, 0.72, 0, 1),
      width 320ms cubic-bezier(0.32, 0.72, 0, 1),
      background 200ms cubic-bezier(0.32, 0.72, 0, 1);
  }
  .c-edge-nav button:hover,
  .c-edge-nav button:focus-visible {
    grid-template-columns: 8px 1fr;
    width: 78px;
    background: var(--proto-surface-strong);
  }
  .c-edge-nav button i {
    width: 6px;
    border: 1px solid currentColor;
    border-radius: 50%;
    aspect-ratio: 1;
  }
  .c-edge-nav button.active {
    color: var(--proto-accent);
  }
  .c-edge-nav button.active i {
    background: currentColor;
    box-shadow: 0 0 0 4px var(--proto-accent-soft);
  }
  .c-edge-nav span {
    overflow: hidden;
    font-size: 0.6rem;
    white-space: nowrap;
  }
  .c-top-actions {
    position: fixed;
    top: 18px;
    right: 18px;
    z-index: 20;
    display: flex;
    gap: 7px;
  }
  .c-command-trigger,
  .c-theme,
  .c-audio-restore {
    display: inline-flex;
    min-height: 38px;
    align-items: center;
    gap: 8px;
    padding: 0 11px;
    border-radius: 13px;
    background: color-mix(in oklab, var(--proto-surface-strong) 78%, transparent);
    color: var(--proto-muted);
    cursor: pointer;
    box-shadow: 0 1px 0 rgb(255 255 255 / 0.06) inset;
    backdrop-filter: blur(14px);
  }
  .c-command-trigger span,
  .c-audio-restore {
    font-size: 0.64rem;
  }
  .c-command-trigger kbd {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
  }
  .c-theme {
    width: 38px;
    justify-content: center;
    padding: 0;
  }
  .c-audio-restore {
    color: var(--proto-accent);
  }
  .c-canvas {
    min-height: 0;
    overflow: auto;
  }
  .c-scene {
    min-height: 100%;
  }

  .c-realm {
    display: grid;
    grid-template-columns: minmax(560px, 1.48fr) minmax(260px, 0.52fr);
    grid-template-rows: 1fr auto;
    gap: 22px 42px;
    width: min(1260px, 100%);
    min-height: 100%;
    margin: auto;
    padding: clamp(72px, 8vw, 112px) clamp(70px, 8vw, 120px) 46px;
  }
  .c-realm-art {
    position: relative;
    min-height: 480px;
  }
  .c-realm-art :global(.cover-art) {
    width: 100%;
    height: 100%;
    aspect-ratio: auto;
  }
  .c-realm-overlay {
    position: absolute;
    right: 7%;
    bottom: 9%;
    left: 7%;
    color: #f6f0e8;
    text-shadow: 0 3px 28px rgb(0 0 0 / 0.42);
  }
  .c-realm-overlay > span,
  .c-novel header > span,
  .c-music-title > span {
    font-family: var(--font-code, monospace);
    font-size: 0.6rem;
    letter-spacing: 0.1em;
  }
  .c-realm-overlay h1 {
    margin-top: 10px;
    font-size: clamp(3.8rem, 7vw, 7.8rem);
    font-weight: 500;
    line-height: 0.82;
    letter-spacing: -0.075em;
  }
  .c-realm-overlay button {
    display: inline-flex;
    min-height: 42px;
    align-items: center;
    gap: 10px;
    margin-top: 24px;
    padding: 0 7px 0 15px;
    border-radius: 999px;
    background: rgb(245 239 231 / 0.92);
    color: #1e2024;
    cursor: pointer;
  }
  .c-realm-overlay button i {
    display: grid;
    width: 28px;
    place-items: center;
    border-radius: 50%;
    background: #d1633f;
    color: #fff;
    transform: rotate(180deg);
    aspect-ratio: 1;
  }
  .c-realm-index {
    align-self: end;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .c-realm-index li {
    border-top: 1px solid var(--proto-line);
  }
  .c-realm-index button {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) 52px;
    gap: 10px;
    align-items: center;
    width: 100%;
    min-height: 88px;
    padding: 10px 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .c-realm-index button > span {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
  }
  .c-realm-index p {
    display: grid;
  }
  .c-realm-index strong {
    font-size: 0.76rem;
  }
  .c-realm-index small {
    margin-top: 3px;
    color: var(--proto-muted);
    font-size: 0.61rem;
  }
  .c-realm-index button > i {
    width: 52px;
    border-radius: 9px;
    background: var(--index-art);
    aspect-ratio: 1.25;
  }
  .c-realm-note {
    grid-column: 1/-1;
    color: var(--proto-muted);
    font-family: var(--font-code, monospace);
    font-size: 0.56rem;
    letter-spacing: 0.04em;
  }

  .c-novel {
    position: relative;
    min-height: 100%;
    padding: clamp(80px, 9vw, 128px) clamp(68px, 8vw, 118px) 54px;
  }
  .c-novel header {
    display: grid;
    grid-template-columns: auto 1fr;
    column-gap: 20px;
    align-items: end;
  }
  .c-novel header > span {
    grid-row: 1/3;
    align-self: start;
    color: var(--proto-accent);
  }
  .c-novel header h1 {
    font-size: clamp(4rem, 8vw, 9rem);
    font-weight: 500;
    line-height: 0.78;
    letter-spacing: -0.08em;
  }
  .c-novel header p {
    margin-top: 15px;
    color: var(--proto-muted);
    font-size: 0.75rem;
  }
  .c-shelf-line {
    position: absolute;
    right: 0;
    bottom: 26%;
    left: 0;
    height: 1px;
    background: var(--proto-line);
  }
  .c-horizontal-shelf {
    position: relative;
    display: grid;
    grid-template-columns: 1.22fr repeat(3, 0.78fr);
    gap: clamp(14px, 2vw, 26px);
    align-items: end;
    margin-top: clamp(34px, 5vh, 72px);
  }
  .c-horizontal-shelf button {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
    padding: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .c-horizontal-shelf button:not(.primary) {
    margin-bottom: 18px;
  }
  .c-horizontal-shelf button > span {
    display: grid;
    margin-top: 11px;
  }
  .c-horizontal-shelf small {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.55rem;
  }
  .c-horizontal-shelf strong {
    margin-top: 3px;
    font-size: 0.78rem;
  }
  .c-horizontal-shelf i {
    overflow: hidden;
    margin-top: 2px;
    color: var(--proto-muted);
    font-size: 0.6rem;
    font-style: normal;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .c-novel-tabs {
    position: absolute;
    top: clamp(92px, 10vw, 140px);
    right: clamp(68px, 8vw, 118px);
    display: flex;
    gap: 4px;
  }
  .c-novel-tabs button {
    min-height: 32px;
    padding: 0 12px;
    border-radius: 999px;
    background: transparent;
    color: var(--proto-muted);
    cursor: pointer;
  }
  .c-novel-tabs button.active {
    background: var(--proto-ink);
    color: var(--proto-canvas);
  }

  .c-music {
    display: grid;
    grid-template-columns: minmax(330px, 0.9fr) minmax(240px, 0.62fr) minmax(300px, 0.8fr);
    gap: clamp(30px, 5vw, 72px);
    align-items: center;
    width: min(1220px, 100%);
    min-height: 100%;
    margin: auto;
    padding: clamp(76px, 8vw, 112px);
  }
  .c-disc {
    position: relative;
    border-radius: 50%;
  }
  .c-disc :global(.cover-art) {
    border-radius: 50%;
  }
  .c-disc > i {
    position: absolute;
    inset: 43%;
    border: 6px solid var(--proto-canvas);
    border-radius: 50%;
    background: var(--proto-accent);
  }
  @media (prefers-reduced-motion: no-preference) {
    .c-disc.playing {
      animation: c-spin 12s linear infinite;
    }
  }
  .c-music-title > span {
    color: var(--proto-accent);
  }
  .c-music-title h1 {
    margin-top: 12px;
    font-size: clamp(4rem, 7vw, 8rem);
    font-weight: 500;
    line-height: 0.8;
    letter-spacing: -0.08em;
  }
  .c-music-title p {
    margin-top: 18px;
    color: var(--proto-muted);
    font-size: 0.72rem;
  }
  .c-music-title button {
    display: inline-flex;
    min-height: 44px;
    align-items: center;
    gap: 8px;
    margin-top: 24px;
    padding: 0 18px;
    border-radius: 999px;
    background: var(--proto-accent);
    color: var(--proto-on-accent);
    cursor: pointer;
  }
  .c-music-queue {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .c-music-queue li {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    gap: 11px;
    align-items: center;
    min-height: 54px;
    border-top: 1px solid var(--proto-line);
    color: var(--proto-muted);
  }
  .c-music-queue li.current {
    color: var(--proto-accent);
  }
  .c-music-queue span,
  .c-music-queue time {
    font-family: var(--font-code, monospace);
    font-size: 0.56rem;
  }
  .c-music-queue strong {
    color: var(--proto-ink);
    font-size: 0.72rem;
  }

  .c-reader-canvas {
    position: relative;
    height: 100%;
    overflow: auto;
    padding: clamp(74px, 10vh, 130px) 24px 150px;
  }
  .c-reader-back {
    position: fixed;
    top: 24px;
    left: 82px;
    z-index: 18;
    display: inline-flex;
    min-height: 38px;
    align-items: center;
    gap: 7px;
    padding: 0 12px;
    border-radius: 13px;
    background: color-mix(in oklab, var(--reader-surface) 86%, transparent);
    color: color-mix(in oklab, var(--reader-ink) 64%, transparent);
    cursor: pointer;
    backdrop-filter: blur(14px);
  }
  .c-reader-canvas article {
    width: min(640px, 100%);
    margin: auto;
    font-family: var(--font-reader, serif);
  }
  .c-reader-canvas article header {
    margin-bottom: 52px;
    text-align: center;
  }
  .c-reader-canvas article header span {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.62rem;
  }
  .c-reader-canvas article h1 {
    margin-top: 12px;
    font-family: var(--font-reader, serif);
    font-size: clamp(2.2rem, 4vw, 3.6rem);
    font-weight: 600;
  }
  .c-reader-canvas article p {
    margin-bottom: 1.22em;
    font-size: clamp(1.04rem, 1.45vw, 1.18rem);
    line-height: 1.9;
  }
  .c-reading-meter {
    position: fixed;
    top: 50%;
    right: 28px;
    display: grid;
    justify-items: center;
    gap: 8px;
    color: color-mix(in oklab, var(--reader-ink) 48%, transparent);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
    transform: translateY(-50%);
  }
  .c-reading-meter b {
    width: 2px;
    height: 90px;
    background: linear-gradient(
      var(--proto-accent) 62%,
      color-mix(in oklab, var(--reader-ink) 10%, transparent) 62%
    );
  }
  .c-reader-audio {
    position: fixed;
    right: 24px;
    bottom: 24px;
  }
  .c-mobile-nav {
    display: none;
  }

  .command-backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgb(6 7 9 / 0.5);
    backdrop-filter: blur(7px);
  }
  .command-palette {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 61;
    width: min(520px, calc(100vw - 28px));
    overflow: hidden;
    border: 1px solid color-mix(in oklab, var(--proto-ink) 12%, transparent);
    border-radius: 22px;
    background: var(--proto-surface-strong);
    color: var(--proto-ink);
    box-shadow: 0 28px 90px rgb(0 0 0 / 0.4);
    transform: translate(-50%, -50%);
  }
  .command-palette header {
    display: grid;
    grid-template-columns: 24px 1fr 38px;
    gap: 12px;
    align-items: center;
    padding: 18px;
    border-bottom: 1px solid var(--proto-line);
  }
  .command-palette header div {
    display: grid;
  }
  .command-palette header span {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.56rem;
    letter-spacing: 0.1em;
  }
  .command-palette h2 {
    font-size: 1.15rem;
    font-weight: 560;
  }
  .command-palette header button {
    display: grid;
    width: 38px;
    place-items: center;
    border-radius: 11px;
    background: color-mix(in oklab, var(--proto-ink) 6%, transparent);
    color: inherit;
    cursor: pointer;
    aspect-ratio: 1;
  }
  .command-list {
    padding: 8px;
  }
  .command-list button {
    display: grid;
    grid-template-columns: 34px 1fr 20px;
    gap: 10px;
    align-items: center;
    width: 100%;
    min-height: 58px;
    padding: 7px 10px;
    border-radius: 13px;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .command-list button:hover,
  .command-list button.active {
    background: var(--proto-accent-soft);
  }
  .command-list kbd {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
  }
  .command-list button > span {
    display: grid;
  }
  .command-list strong {
    font-size: 0.76rem;
  }
  .command-list small {
    margin-top: 2px;
    color: var(--proto-muted);
    font-size: 0.6rem;
  }
  .command-list button > i {
    color: var(--proto-muted);
    font-style: normal;
  }
  .command-palette footer {
    padding: 12px 18px 16px;
    border-top: 1px solid var(--proto-line);
    color: var(--proto-muted);
    font-size: 0.6rem;
  }

  @keyframes c-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 900px) {
    .c-realm {
      grid-template-columns: 1.35fr 0.65fr;
      padding: 88px 72px 42px;
    }
    .c-realm-art {
      min-height: 430px;
    }
    .c-realm-index button {
      grid-template-columns: 20px 1fr;
    }
    .c-realm-index button > i {
      display: none;
    }
    .c-novel {
      padding-inline: 72px;
    }
    .c-horizontal-shelf {
      grid-template-columns: 1.2fr repeat(2, 0.8fr);
    }
    .c-horizontal-shelf button:last-child {
      display: none;
    }
    .c-music {
      grid-template-columns: 0.9fr 0.8fr;
      padding-inline: 72px;
    }
    .c-music-queue {
      grid-column: 1/-1;
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      gap: 0 24px;
    }
  }

  @media (max-width: 680px) {
    .variant-c {
      grid-template-rows: minmax(0, 1fr) auto auto;
    }
    .variant-c.reader {
      display: grid;
      grid-template-rows: minmax(0, 1fr) auto;
    }
    .c-mark {
      top: 10px;
      left: 10px;
      width: 40px;
      border-radius: 12px;
    }
    .c-mark small,
    .c-edge-nav,
    .c-command-trigger span,
    .c-command-trigger kbd,
    .c-audio-restore {
      display: none;
    }
    .c-top-actions {
      top: 10px;
      right: 10px;
    }
    .c-command-trigger {
      width: 38px;
      justify-content: center;
      padding: 0;
    }
    .c-mobile-nav {
      display: grid;
      grid-template-columns: repeat(4, 1fr);
      min-height: calc(60px + env(safe-area-inset-bottom, 0px));
      padding: 4px 8px env(safe-area-inset-bottom, 0px);
      background: var(--proto-canvas);
    }
    .reader .c-mobile-nav {
      display: none;
    }
    .c-mobile-nav button {
      display: grid;
      place-items: center;
      align-content: center;
      gap: 3px;
      border-radius: 12px;
      background: transparent;
      color: var(--proto-muted);
    }
    .c-mobile-nav button.active {
      color: var(--proto-accent);
    }
    .c-mobile-nav span {
      font-size: 0.58rem;
    }
    .c-realm {
      display: block;
      padding: 62px 12px 40px;
    }
    .c-realm-art {
      min-height: 430px;
    }
    .c-realm-overlay {
      right: 20px;
      bottom: 24px;
      left: 20px;
    }
    .c-realm-overlay h1 {
      font-size: clamp(3.6rem, 17vw, 5.2rem);
    }
    .c-realm-index {
      margin-top: 18px;
    }
    .c-realm-index li:nth-child(n + 3) {
      display: none;
    }
    .c-realm-note {
      margin-top: 18px;
      line-height: 1.5;
    }
    .c-novel {
      padding: 72px 14px 42px;
    }
    .c-novel header {
      display: block;
    }
    .c-novel header > span {
      display: block;
    }
    .c-novel header h1 {
      margin-top: 9px;
      font-size: 4.8rem;
    }
    .c-novel-tabs {
      top: 18px;
      right: 62px;
    }
    .c-horizontal-shelf {
      grid-template-columns: 1.15fr 0.85fr;
      gap: 12px;
      margin-top: 28px;
    }
    .c-horizontal-shelf button:nth-child(n + 3) {
      display: none;
    }
    .c-shelf-line {
      bottom: 22%;
    }
    .c-music {
      display: grid;
      grid-template-columns: 1fr 1fr;
      padding: 64px 16px 40px;
    }
    .c-disc {
      grid-column: 1/-1;
      width: min(74vw, 320px);
      margin: auto;
    }
    .c-music-title h1 {
      font-size: 4.2rem;
    }
    .c-music-queue {
      grid-column: 1/-1;
      display: block;
    }
    .c-music-queue li:nth-child(n + 4) {
      display: none;
    }
    .c-reader-canvas {
      padding: 70px 22px 132px;
    }
    .c-reader-back {
      top: 12px;
      left: 60px;
    }
    .c-reader-canvas article header {
      margin-bottom: 38px;
    }
    .c-reader-canvas article h1 {
      font-size: 2.2rem;
    }
    .c-reader-canvas article p {
      font-size: 1rem;
      line-height: 1.84;
    }
    .c-reading-meter {
      display: none;
    }
    .c-reader-audio {
      right: 50%;
      bottom: 82px;
      transform: translateX(50%);
    }
    .command-palette {
      top: auto;
      right: 7px;
      bottom: 7px;
      left: 7px;
      width: auto;
      transform: none;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .c-mark,
    .c-command-trigger,
    .c-theme,
    .c-reader-back,
    .command-backdrop {
      backdrop-filter: none;
    }
    .c-mark,
    .c-command-trigger,
    .c-theme,
    .c-reader-back {
      background: var(--proto-surface-strong);
    }
  }
</style>
