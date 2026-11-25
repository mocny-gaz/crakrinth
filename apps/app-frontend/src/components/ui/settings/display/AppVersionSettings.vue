<script setup lang="ts">
import { DownloadIcon } from '@modrinth/assets'
import { Button, Combobox, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { getVersion } from '@tauri-apps/api/app'
import { computed, onMounted, ref } from 'vue'

import {
	compareAppVersions,
	forkReleases,
	installForkRelease,
	refreshForkReleases,
} from '@/helpers/fork-releases.ts'

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
	refresh: {
		id: 'app.version-settings.refresh',
		defaultMessage: 'Refresh list',
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
const refreshing = ref(false)
const installing = ref(false)

const options = computed(() =>
	forkReleases.value.map(({ version }) => ({
		value: version,
		label: version === current.value ? formatMessage(messages.current, { version }) : version,
	})),
)

const installLabel = computed(() => {
	if (installing.value) return formatMessage(messages.installing)
	if (!selected.value) return formatMessage(messages.select)
	const version = selected.value
	return formatMessage(
		compareAppVersions(version, current.value) < 0 ? messages.downgrade : messages.upgrade,
		{ version },
	)
})

async function refresh() {
	refreshing.value = true
	try {
		await refreshForkReleases()
	} catch (error) {
		handleError(error as Error)
	}
	refreshing.value = false
}

async function install() {
	const release = forkReleases.value.find(({ version }) => version === selected.value)
	if (!release) return
	installing.value = true
	try {
		await installForkRelease(release)
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
					:disabled="!selected || selected === current || installing || refreshing"
					@click="install()"
				>
					<DownloadIcon />
					{{ installLabel }}
				</Button>
				<Button type="outlined" :disabled="refreshing || installing" @click="refresh()">
					{{ formatMessage(messages.refresh) }}
				</Button>
			</div>
		</div>
	</section>
</template>
