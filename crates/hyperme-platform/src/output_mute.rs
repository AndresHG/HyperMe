//! Mutes the Mac's sound while the user dictates (#221, AC-7), through Core Audio's public
//! mute property on the default output device. Nothing is paused, so nothing can start playing
//! by mistake, and it works for every app. The device and channels muted are remembered, so the
//! unmute gives back exactly what was taken, even if the default output changed meanwhile.

use std::ffi::c_void;
use std::ptr::NonNull;

use hyperme_core::microphones::OutputMute;
use objc2_core_audio::{
    AudioObjectGetPropertyData, AudioObjectHasProperty, AudioObjectID,
    AudioObjectIsPropertySettable, AudioObjectPropertyAddress, AudioObjectSetPropertyData,
    kAudioDevicePropertyMute, kAudioHardwarePropertyDefaultOutputDevice,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject,
};

/// Devices without a mute on the main element may have one on each channel. Channels past two
/// are rare on an output a person listens to.
const CHANNELS: [u32; 2] = [1, 2];

/// The default output's mute, given back after each dictation.
#[derive(Debug, Default)]
pub struct SystemOutputMute {
    /// The device and elements the last `mute` switched on.
    taken: Option<(AudioObjectID, Vec<u32>)>,
}

fn address(selector: u32, scope: u32, element: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: element,
    }
}

#[allow(unsafe_code)]
fn default_output() -> Option<AudioObjectID> {
    let mut address = address(
        kAudioHardwarePropertyDefaultOutputDevice,
        kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyElementMain,
    );
    let mut device: AudioObjectID = 0;
    let mut size = size_of::<AudioObjectID>() as u32;
    // SAFETY: the address, size and output buffer are valid for the call, and the buffer is
    // the size given.
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&mut address),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut device).cast::<c_void>(),
        )
    };
    (status == 0 && device != 0).then_some(device)
}

/// The mute switch of `device` on `element`, if it has one that can be set.
#[allow(unsafe_code)]
fn settable(device: AudioObjectID, element: u32) -> bool {
    let mut address = address(
        kAudioDevicePropertyMute,
        kAudioObjectPropertyScopeOutput,
        element,
    );
    // SAFETY: the address is valid for the calls and the out flag is a valid u8.
    unsafe {
        if !AudioObjectHasProperty(device, NonNull::from(&mut address)) {
            return false;
        }
        let mut can: u8 = 0;
        AudioObjectIsPropertySettable(device, NonNull::from(&mut address), NonNull::from(&mut can))
            == 0
            && can != 0
    }
}

#[allow(unsafe_code)]
fn muted(device: AudioObjectID, element: u32) -> Option<bool> {
    let mut address = address(
        kAudioDevicePropertyMute,
        kAudioObjectPropertyScopeOutput,
        element,
    );
    let mut value: u32 = 0;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: as in `default_output`.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            NonNull::from(&mut address),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast::<c_void>(),
        )
    };
    (status == 0).then_some(value != 0)
}

#[allow(unsafe_code)]
fn set_muted(device: AudioObjectID, element: u32, on: bool) -> bool {
    let mut address = address(
        kAudioDevicePropertyMute,
        kAudioObjectPropertyScopeOutput,
        element,
    );
    let mut value = u32::from(on);
    // SAFETY: the address and the u32 value are valid for the call and the size is the value's.
    let status = unsafe {
        AudioObjectSetPropertyData(
            device,
            NonNull::from(&mut address),
            0,
            std::ptr::null(),
            size_of::<u32>() as u32,
            NonNull::from(&mut value).cast::<c_void>(),
        )
    };
    status == 0
}

impl OutputMute for SystemOutputMute {
    fn mute(&mut self) -> bool {
        let Some(device) = default_output() else {
            return false;
        };
        // Only the switches that are off now: one the user had on stays theirs.
        let elements = if settable(device, kAudioObjectPropertyElementMain) {
            vec![kAudioObjectPropertyElementMain]
        } else {
            CHANNELS
                .into_iter()
                .filter(|&e| settable(device, e))
                .collect()
        };
        let taken: Vec<u32> = elements
            .into_iter()
            .filter(|&e| muted(device, e) == Some(false))
            .filter(|&e| set_muted(device, e, true))
            .collect();
        if taken.is_empty() {
            // Already muted, or a device without a mute switch (some HDMI and USB outputs).
            return false;
        }
        self.taken = Some((device, taken));
        true
    }

    fn unmute(&mut self) {
        if let Some((device, elements)) = self.taken.take() {
            for element in elements {
                // A device unplugged meanwhile fails here, which is fine.
                set_muted(device, element, false);
            }
        }
    }
}
