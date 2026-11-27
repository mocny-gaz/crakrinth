<script setup lang="ts">
import { FolderOpenIcon } from '@modrinth/assets'
import {
	Button,
	defineMessages,
	FileTreeSelect,
	injectNotificationManager,
	useVIntl,
} from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref } from 'vue'

import { importModrinthFolder, listModrinthImportInstances } from '@/helpers/utils.js'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	title: {
		id: 'app.import-settings.title',
		defaultMessage: 'Import from Modrinth App',
	},
	description: {
		id: 'app.import-settings.description',
		defaultMessage:
			'Copies instances, accounts and settings from another Modrinth App data folder. That folder is not changed. Your current database is backed up first.',
	},
	choose: {
		id: 'app.import-settings.choose',
		defaultMessage: 'Choose folder',
	},
	importing: {
		id: 'app.import-settings.importing',
		defaultMessage: 'Importing...',
	},
	pickerTitle: {
		id: 'app.import-settings.picker-title',
		defaultMessage: 'Select the Modrinth App data folder',
	},
	done: {
		id: 'app.import-settings.done',
		defaultMessage:
			'Imported {instances, plural, one {# instance} other {# instances}} and {accounts, plural, one {# account} other {# accounts}}. Restart the app to finish.',
	},
	restart: {
		id: 'app.import-settings.restart',
		defaultMessage: 'Restart app',
	},
	everything: {
		id: 'app.import-settings.everything',
		defaultMessage:
			'Everything will be imported: {count, plural, one {# instance} other {# instances}}, accounts and settings.',
	},
	chooseWhat: {
		id: 'app.import-settings.choose-what',
		defaultMessage: 'Choose what to import',
	},
	importAll: {
		id: 'app.import-settings.import-all',
		defaultMessage: 'Import everything',
	},
	importSelected: {
		id: 'app.import-settings.import-selected',
		defaultMessage: 'Import {count, plural, one {# instance} other {# instances}}',
	},
	noInstances: {
		id: 'app.import-settings.no-instances',
		defaultMessage: 'This folder has no instances. Accounts and settings can still be imported.',
	},
})

type ImportableInstance = { id: string; name: string; path: string }

const folder = ref<string | null>(null)
const instances = ref<ImportableInstance[]>([])
const picking = ref(false)
const includedPaths = ref<string[]>([])
const excludedPaths = ref<string[]>([])
const importing = ref(false)
const summary = ref<{ instances: number; accounts: number } | null>(null)

const items = computed(() =>
	instances.value.map((instance) => ({ path: instance.path, type: 'file' as const })),
)
const selectedIds = computed(() => {
	const included = new Set(includedPaths.value)
	const excluded = new Set(excludedPaths.value)
	return instances.value
		.filter((instance) => included.has(instance.path) && !excluded.has(instance.path))
		.map((instance) => instance.id)
})

async function chooseFolder() {
	const chosen = await open({
		multiple: false,
		directory: true,
		title: formatMessage(messages.pickerTitle),
	})
	if (!chosen) return

	try {
		instances.value = await listModrinthImportInstances(chosen)
	} catch (error) {
		handleError(error as Error)
		return
	}
	folder.value = chosen
	summary.value = null
	picking.value = false
	includedPaths.value = instances.value.map((instance) => instance.path)
	excludedPaths.value = []
}

async function startImport() {
	if (!folder.value) return
	importing.value = true
	try {
		summary.value = await importModrinthFolder(
			folder.value,
			picking.value ? selectedIds.value : null,
		)
		folder.value = null
	} catch (error) {
		handleError(error as Error)
	}
	importing.value = false
}
</script>
<template>
	<section>
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.title) }}
		</h2>
		<p class="m-0 mt-2">{{ formatMessage(messages.description) }}</p>
		<div class="mt-4 flex gap-2">
			<Button type="outlined" :disabled="importing" @click="chooseFolder()">
				<FolderOpenIcon />
				{{ formatMessage(messages.choose) }}
			</Button>
			<Button v-if="summary" type="outlined" @click="invoke('restart_app')">
				{{ formatMessage(messages.restart) }}
			</Button>
		</div>
		<div v-if="folder" class="mt-4 flex flex-col gap-3">
			<p class="m-0 break-all text-secondary">{{ folder }}</p>
			<p v-if="instances.length === 0" class="m-0">{{ formatMessage(messages.noInstances) }}</p>
			<p v-else-if="!picking" class="m-0">
				{{ formatMessage(messages.everything, { count: instances.length }) }}
			</p>
			<button
				v-if="instances.length > 0"
				type="button"
				class="w-fit cursor-pointer appearance-none border-0 bg-transparent p-0 font-semibold text-brand hover:underline"
				@click="picking = !picking"
			>
				{{ formatMessage(messages.chooseWhat) }}
			</button>
			<FileTreeSelect
				v-if="picking"
				v-model="includedPaths"
				v-model:excluded-paths="excludedPaths"
				class="min-w-0"
				:items="items"
				:show-size="false"
				:show-modified="false"
			/>
			<div>
				<Button
					type="colored"
					color="brand"
					:disabled="importing || (picking && selectedIds.length === 0 && instances.length > 0)"
					@click="startImport()"
				>
					{{
						importing
							? formatMessage(messages.importing)
							: picking
								? formatMessage(messages.importSelected, { count: selectedIds.length })
								: formatMessage(messages.importAll)
					}}
				</Button>
			</div>
		</div>
		<p v-if="summary" class="m-0 mt-4">{{ formatMessage(messages.done, summary) }}</p>
	</section>
</template>
