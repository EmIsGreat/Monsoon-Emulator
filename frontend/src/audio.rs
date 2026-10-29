use std::error::Error;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ChannelCount, SAMPLE_RATE_48K, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};

pub struct CpalAudio {
    _stream: cpal::Stream,
}

impl CpalAudio {
    pub fn new() -> Result<(Self, Producer<f32>), Box<dyn Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No audio output device available")?;

        let (producer, consumer) = RingBuffer::new(4800);

        let supported_config = device
            .supported_output_configs()?
            .find(|config| {
                config.sample_format() == cpal::SampleFormat::F32
                    && config.min_sample_rate() <= SAMPLE_RATE_48K
                    && config.max_sample_rate() >= SAMPLE_RATE_48K
                    && config.channels() >= 1
            })
            .ok_or("No f32 output configuration supports 48KHz")?
            .with_sample_rate(SAMPLE_RATE_48K);

        let channels = supported_config.channels();
        let config: StreamConfig = supported_config.into();

        let stream = Self::build_stream(&device, &config, channels, consumer)?;

        stream.play()?;

        Ok((
            Self {
                _stream: stream,
            },
            producer,
        ))
    }

    fn build_stream(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        channel_count: ChannelCount,
        mut consumer: Consumer<f32>,
    ) -> Result<cpal::Stream, Box<dyn Error>> {
        let stream = device.build_output_stream(
            *config,
            move |output: &mut [f32], _| {
                for frame in output.chunks_mut(channel_count as usize) {
                    let sample = consumer.pop().unwrap_or(0.0);

                    for channel in frame {
                        *channel = sample;
                    }
                }
            },
            |err| eprintln!("Audio stream error: {err}"),
            None,
        )?;

        Ok(stream)
    }
}
