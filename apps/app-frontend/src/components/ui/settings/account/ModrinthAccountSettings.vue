<script setup lang="ts">
import { defineMessages, Toggle, useVIntl } from '@modrinth/ui'

import { modrinthSignInEnabled, setModrinthSignInEnabled } from '@/helpers/modrinth-account.ts'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'app.settings.modrinth-account.title',
		defaultMessage: 'Modrinth account sign in',
	},
	description: {
		id: 'app.settings.modrinth-account.description',
		defaultMessage:
			'Crakrinth works without a Modrinth account. Turn this on only if you want friends, shared instances or your Modrinth profile.',
	},
	warning: {
		id: 'app.settings.modrinth-account.warning',
		defaultMessage:
			'Signing in uses your Modrinth account. Your login goes to modrinth.com and a session token is stored on this computer. Crakrinth is a fork that Modrinth does not review, so only turn this on if you trust it. Turning it off signs you out.',
	},
})
</script>

<template>
	<div class="flex items-center justify-between gap-4">
		<div>
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.description) }}</p>
			<p class="m-0 mt-2 font-semibold text-orange">{{ formatMessage(messages.warning) }}</p>
		</div>
		<Toggle
			id="modrinth-sign-in"
			:model-value="modrinthSignInEnabled"
			@update:model-value="(value) => setModrinthSignInEnabled(!!value)"
		/>
	</div>
</template>
