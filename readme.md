# Habit Tracker

Status: in development, not usable yet.

## Home-assistant integration

```yaml
rest:
  - resource: http://IP_ADDRESS:4124/api/current
    scan_interval: 60
    binary_sensor:
      - name: Workout streak alive
        value_template: "{{ value_json['active'] }}"
      - name: Workout active today
        value_template: "{{ value_json['active_today'] }}"
    sensor:
      - name: Most recent workout
        value_template: "{{ value_json['end'] }}"
        device_class: "timestamp"
      - name: Workout streak length
        value_template: "{{ value_json['days'] }}"
        unit_of_measurement: "d"
```
