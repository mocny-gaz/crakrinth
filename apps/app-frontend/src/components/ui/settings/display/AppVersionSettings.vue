<script setup lang="ts">
import { DownloadIcon } from '@modrinth/assets'
import { Button, Combobox, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { getVersion } from '@tauri-apps/api/app'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, onMounted, ref } from 'vue'

import {
	checkAppVersion,
	enqueueUpdateForInstallation,
	setRestartAfterPendingUpdate,
} from '@/helpers/utils.js'

const RELEASES_URL = 'https://api.github.com/repos/mocny-gaz/crakrinth/releases?per_page=100'
const STORAGE_KEY = 'app_versions'
const TAG_PREFIX = 'crak-v'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	title: {
		id: 'app.version-settings.title',
		defaultMessage: 'App version',
	},
	description: {
		id: 'app.version-settings.description',
		defaultMessage:
			'You are on version {version}. Pick any listed version to downgrade or upgrade. The app restarts to install it.',
	},
	placeholder: {
		id: 'app.version-settings.placeholder',
		defaultMessage: 'Select a version',
	},
	current: {
		id: 'app.version-settings.current',
		defaultMessage: '{version} (current)',
	},
	check: {
		id: 'app.version-settings.check',
		defaultMessage: 'Check for updates',
	},
	upgrade: {
		id: 'app.version-settings.upgrade',
		defaultMessage: 'Upgrade to {version}',
	},
	downgrade: {
		id: 'app.version-settings.downgrade',
		defaultMessage: 'Downgrade to {version}',
	},
	select: {
		id: 'app.version-settings.select',
		defaultMessage: 'Install selected version',
	},
	installing: {
		id: 'app.version-settings.installing',
		defaultMessage: 'Installing...',
	},
})

const current = ref('')
const selected = ref('')
const checking = ref(false)
const installing = ref(false)

function compare(a: string, b: string) {
	return a.localeCompare(b, undefined, { numeric: true })
}

function loadVersions(): string[] {
	try {
		return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]')
	} catch {
		return []
	}
}

const versions = ref<string[]>(loadVersions())

const options = computed(() =>
	versions.value.map((version) => ({
		value: version,
		label: version === current.value ? formatMessage(messages.current, { version }) : version,
	})),
)

const installLabel = computed(() => {
	if (installing.value) return formatMessage(messages.installing)
	if (!selected.value) return formatMessage(messages.select)
	const version = selected.value
	return formatMessage(
		compare(version, current.value) < 0 ? messages.downgrade : messages.upgrade,
		{ version },
	)
})

async function checkForUpdates() {
	checking.value = true
	try {
		const response = await fetch(RELEASES_URL)
		if (!response.ok) throw new Error(`GitHub returned ${response.status}`)
		const releases: { tag_name: string }[] = await response.json()
		versions.value = releases
			.map((release) => release.tag_name)
			.filter((tag) => tag.startsWith(TAG_PREFIX))
			.map((tag) => tag.slice(TAG_PREFIX.length))
			.sort((a, b) => compare(b, a))
		localStorage.setItem(STORAGE_KEY, JSON.stringify(versions.value))
	} catch (error) {
		handleError(error as Error)
	}
	checking.value = false
}

async function install() {
	installing.value = true
	try {
		const update = await checkAppVersion(selected.value)
		await enqueueUpdateForInstallation(update.rid)
		await setRestartAfterPendingUpdate(true)
		await getCurrentWindow().close()
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
	<section>
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.title) }}
		</h2>
		<p class="m-0 mt-2">{{ formatMessage(messages.description, { version: current }) }}</p>
		<div class="mt-4 flex flex-col gap-4">
			<Combobox
				v-model="selected"
				:options="options"
				:placeholder="formatMessage(messages.placeholder)"
			/>
			<div class="flex gap-2">
				<Button
					type="colored"
					color="brand"
					:disabled="!selected || selected === current || installing || checking"
					@click="install()"
				>
					<DownloadIcon />
					{{ installLabel }}
				</Button>
				<Button type="outlined" :disabled="checking || installing" @click="checkForUpdates()">
					{{ formatMessage(messages.check) }}
				</Button>
			</div>
		</div>
	</section>
</template>
