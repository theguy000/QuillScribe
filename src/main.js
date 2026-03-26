import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'

// Disable right-click context menu
document.addEventListener('contextmenu', (e) => e.preventDefault())

const app = mount(App, {
  target: document.getElementById('app'),
})

export default app
