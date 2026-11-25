<script setup lang="ts">
import { DownloadIcon, RefreshCwIcon } from '@modrinth/assets'
import { Button, defineMessages, injectNotificationManager, Toggle, useVIntl } from '@modrinth/ui'
import { getVersion } from '@tauri-apps/api/app'
import { computed, onMounted, ref, watch } from 'vue'

import AppVersionSettings from '@/components/ui/settings/display/AppVersionSettings.vue'
import {
	forkReleases,
	installForkRelease,
	newerBuildsOfVersion,
	parseAppVersion,
	refreshForkReleases,
} from '@/helpers/fork-releases.ts'
import { get, set } from '@/helpers/settings.ts'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	autoUpdatesTitle: {
		id: 'app.update-settings.auto-updates.title',
		defaultMessage: 'Automatic updates',
	},
	autoUpdatesDescription: {
		id: 'app.update-settings.auto-updates.description',
		defaultMessage:
			'Check for, download and install app updates automatically. When off, the app only checks when you press Check for updates.',
	},
	checkTitle: {
		id: 'app.update-settings.check.title',
		defaultMessage: 'Fork updates',
	},
	checkDescription: {
		id: 'app.update-settings.check.description',
		defaultMessage:
			'Looks for newer Crakrinth builds of version {base}. They add new fork features and fixes on top of the same base version.',
	},
	check: {
		id: 'app.update-settings.check.button',
		defaultMessage: 'Check for updates',
	},
	checking: {
		id: 'app.update-settings.check.checking',
		defaultMessage: 'Checking...',
	},
	upToDate: {
		id: 'app.update-settings.check.up-to-date',
		defaultMessage: 'You have the latest build of {base} ({version}).',
	},
	available: {
		id: 'app.update-settings.check.available',
		defaultMessage: 'Build {version} is available.',
	},
	noNotes: {
		id: 'app.update-settings.check.no-notes',
		defaultMessage: 'No release notes were published for this build.',
	},
	install: {
		id: 'app.update-settings.check.install',
		defaultMessage: 'Install {version}',
	},
	installing: {
		id: 'app.update-settings.check.installing',
		defaultMessage: 'Installing...',
	},
})

const settings = ref(await get())

watch(
	settings,
	async () => {
		await set(settings.value)
	},
	{ deep: true },
)

const current = ref('')
const checking = ref(false)
const installing = ref(false)
const checked = ref(false)

const base = computed(() => parseAppVersion(current.value).base)
const newerBuilds = computed(() => newerBuildsOfVersion(forkReleases.value, current.value))
const newest = computed(() => newerBuilds.value[0])

async function checkForUpdates() {
	checking.value = true
	try {
		await refreshForkReleases()
		checked.value = true
	} catch (error) {
		handleError(error as Error)
	}
	checking.value = false
}

async function installNewest() {
	if (!newest.value) return
	installing.value = true
	try {
		await installForkRelease(newest.value)
	} catch (error) {
		handleError(error as Error)
		installing.value = false
	}
}

onMounted(async () => {
	current.value = await getVersion()
})
</script>
<template>
	<div class="flex flex-col gap-8">
		<section>
			<div class="flex items-center justify-between gap-4">
				<div>
					<h2 class="m-0 text-xl font-semibold text-contrast">
						{{ formatMessage(messages.autoUpdatesTitle) }}
					</h2>
					<p class="m-0 mt-1">{{ formatMessage(messages.autoUpdatesDescription) }}</p>
				</div>
				<Toggle
					id="auto-updates"
					:model-value="!!settings.auto_download_updates"
					@update:model-value="(e) => (settings.auto_download_updates = !!e)"
				/>
			</div>
		</section>

		<section>
			<h2 class="m-0 text-xl font-semibold text-contrast">
				{{ formatMessage(messages.checkTitle) }}
			</h2>
			<p class="m-0 mt-2">{{ formatMessage(messages.checkDescription, { base }) }}</p>
			<div class="mt-4 flex flex-col gap-4">
				<div>
					<Button type="outlined" :disabled="checking || installing" @click="checkForUpdates()">
						<RefreshCwIcon :class="{ 'animate-spin': checking }" />
						{{ formatMessage(checking ? messages.checking : messages.check) }}
					</Button>
				</div>
				<p v-if="checked && !newest" class="m-0 font-semibold text-contrast">
					{{ formatMessage(messages.upToDate, { base, version: current }) }}
				</p>
				<div v-for="release in newerBuilds" :key="release.version" class="flex flex-col gap-1">
					<p class="m-0 font-semibold text-contrast">
						{{ formatMessage(messages.available, { version: release.version }) }}
					</p>
					<p class="m-0 whitespace-pre-line">
						{{ release.notes || formatMessage(messages.noNotes) }}
					</p>
				</div>
				<div v-if="newest">
					<Button type="colored" color="brand" :disabled="installing" @click="installNewest()">
						<DownloadIcon />
						{{
							installing
								? formatMessage(messages.installing)
								: formatMessage(messages.install, { version: newest.version })
						}}
					</Button>
				</div>
			</div>
		</section>

		<AppVersionSettings />
	</div>
</template>
