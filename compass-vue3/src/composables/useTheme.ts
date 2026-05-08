import { ref } from 'vue'

export type Theme = 'light' | 'dark'

const theme = ref<Theme>(
  (localStorage.getItem('compass-theme') as Theme) ??
  (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
)

export function useTheme() {
  const applyTheme = (t: Theme) => {
    theme.value = t
    document.documentElement.setAttribute('data-theme', t)
    localStorage.setItem('compass-theme', t)
  }

  const toggleTheme = () => {
    applyTheme(theme.value === 'dark' ? 'light' : 'dark')
  }

  // Watch for system preference changes
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
  mediaQuery.addEventListener('change', (e) => {
    if (!localStorage.getItem('compass-theme')) {
      applyTheme(e.matches ? 'dark' : 'light')
    }
  })

  return { theme, toggleTheme, applyTheme }
}
