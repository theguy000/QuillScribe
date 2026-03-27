import { mount } from 'svelte'
import './app.css'
import RecordingOverlay from './lib/RecordingOverlay.svelte'

document.addEventListener('contextmenu', (e) => e.preventDefault())

// Force transparent background — overrides app.css :root background
document.documentElement.style.setProperty('background', 'transparent', 'important')
document.body.style.setProperty('background', 'transparent', 'important')

const app = mount(RecordingOverlay, {
  target: document.getElementById('overlay'),
})

export default app
