import { createApp } from 'vue'
import { createPinia } from 'pinia'
import router from './router'
import App from './App.vue'
import './styles/variables.css'
import './style.css'

// Apply saved or system theme before first paint to avoid FOUC
const savedTheme = localStorage.getItem('compass-theme')
const systemPrefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
const theme = savedTheme ?? (systemPrefersDark ? 'dark' : 'light')
document.documentElement.setAttribute('data-theme', theme)

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.mount('#app')
