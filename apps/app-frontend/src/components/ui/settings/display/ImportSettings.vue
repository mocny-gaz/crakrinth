<script setup lang="ts">
import { FolderOpenIcon } from '@modrinth/assets'
import { Button, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { ref } from 'vue'

import { importModrinthFolder } from '@/helpers/utils.js'

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
})

const importing = ref(false)
const summary = ref<{ instances: number; accounts: number } | null>(null)

async function chooseFolder() {
	const folder = await open({
		multiple: false,
		directory: true,
		title: formatMessage(messages.pickerTitle),
	})
	if (!folder) return

	importing.value = true
	try {
		summary.value = await importModrinthFolder(folder)
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
			<Button type="colored" color="brand" :disabled="importing" @click="chooseFolder()">
				<FolderOpenIcon />
				{{ importing ? formatMessage(messages.importing) : formatMessage(messages.choose) }}
			</Button>
			<Button v-if="summary" type="outlined" @click="invoke('restart_app')">
				{{ formatMessage(messages.restart) }}
			</Button>
		</div>
		<p v-if="summary" class="m-0 mt-4">{{ formatMessage(messages.done, summary) }}</p>
	</section>
</template>
