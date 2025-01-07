import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Song } from "~/types/types";

export default defineNuxtPlugin((nuxtApp) => {
  const currentSong = ref<Song | null>(null);
  const duration = ref(0);
  const looping = ref(false);
  const muted = ref(false);
  const paused = ref(true);
  const progress = ref(0);
  const time = ref(0);
  const volume = ref(50);

  const player = {
    currentSong,
    duration,
    looping,
    muted,
    paused,
    progress,
    time,
    volume,

    async loadSong(song: Song) {
      this.currentSong.value = song;
      await invoke("load_song", { song });
    },

    async play() {
      await invoke("play");
    },

    async pause() {
      await invoke("pause");
    },

    async playPause() {
      await invoke("play_pause");
    },

    async rewind() {
      await invoke("rewind");
    },

    async setLooping(looping: boolean) {
      this.looping.value = looping;
      await invoke("set_looping", { looping });
    },

    async setMuted(muted: boolean) {
      this.muted.value = muted;
      await invoke("set_muted", { muted });
    },

    async setVolume(volume: number) {
      this.volume.value = volume;
      await invoke("set_volume", { volume });
    },

    async skip() {
      await invoke("skip");
    },

    async skipTo(percentage: number) {
      await invoke("skip_to", { percentage });
    },

    async setEqSettings(settings: any) {
      await invoke("set_eq_settings", { settings });
    },

    async seek(position: number) {
    await invoke("seek", { position });
    },
  };

  listen("player-update", (event: any) => {
    const { duration, paused, progress, time } = event.payload;
    player.duration.value = duration;
    player.paused.value = paused;
    player.progress.value = progress;
    player.time.value = time;
  });

  return {
    provide: {
      player,
    },
  };
});
