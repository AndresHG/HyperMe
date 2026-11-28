// The walkthrough (onboarding.rs, walkthrough.rs): five screens that get a new user to a first
// dictation.
import { mount } from 'svelte';
import '../../../ui/tokens.css';
import '../../../ui/components.css';
import './onboarding.css';
import Onboarding from './Onboarding.svelte';

mount(Onboarding, { target: document.body });
