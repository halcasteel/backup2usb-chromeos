import { memo } from 'react';
import {
  Box,
  Flex,
  Heading,
  HStack,
  VStack,
  Text,
  Badge,
  Stat,
  StatLabel,
  StatNumber,
  StatGroup,
  useColorModeValue,
} from '@chakra-ui/react';
import { BackupStatus } from '../types/backup';
import { formatBytes, formatDuration } from '../utils/format';

interface HeaderProps {
  status: BackupStatus | null;
}

export function Header({ status }: HeaderProps) {
  const bgColor = useColorModeValue('gray.50', 'gray.800');
  const borderColor = useColorModeValue('gray.200', 'gray.700');
  const statBg = useColorModeValue('white', 'gray.700');
  const accentColor = 'brand.500';

  if (!status) {
    return (
      <Box bg={bgColor} borderBottom="1px" borderColor={borderColor} py={4}>
        <Flex px={6} align="center" justify="space-between">
          <Heading size="lg" color={accentColor}>
            Backup Operations Dashboard
          </Heading>
        </Flex>
      </Box>
    );
  }

  const completedCount = status.directories.filter(d => d.status === 'completed').length;
  const speed = status.state === 'running' ? calculateSpeed(status) : 0;
  const eta = status.state === 'running' ? calculateETA(status, speed) : null;

  return (
    <Box bg={bgColor} borderBottom="1px" borderColor={borderColor} py={4}>
      <Flex px={6} direction="column" gap={4}>
        <Flex align="center" justify="space-between">
          <HStack spacing={4}>
            <Heading size="lg" fontWeight="600">
              Backup Operations Dashboard
            </Heading>
            {status.state === 'running' && (
              <Badge colorScheme="green" variant="subtle" fontSize="sm">
                ACTIVE
              </Badge>
            )}
            {status.state === 'paused' && (
              <Badge colorScheme="yellow" variant="subtle" fontSize="sm">
                PAUSED
              </Badge>
            )}
          </HStack>

          <HStack spacing={8}>
            <VStack spacing={0} align="flex-end">
              <Text fontSize="xs" color="gray.500" fontWeight="medium">
                SOURCE DISK
              </Text>
              <HStack spacing={1}>
                <Text fontSize="sm" fontWeight="bold" color={
                  (status.localDiskSpace?.percentage || 0) > 90 ? 'red.500' : accentColor
                }>
                  {formatBytes(status.localDiskSpace?.free || 0)}
                </Text>
                <Text fontSize="xs" color="gray.500">
                  ({status.localDiskSpace?.percentage.toFixed(1)}% used)
                </Text>
              </HStack>
            </VStack>

            <VStack spacing={0} align="flex-end">
              <Text fontSize="xs" color="gray.500" fontWeight="medium">
                BACKUP DISK
              </Text>
              <HStack spacing={1}>
                <Text fontSize="sm" fontWeight="bold" color={
                  !status.remoteDiskSpace ? 'red.500' : 
                  status.remoteDiskSpace.percentage > 90 ? 'orange.500' : accentColor
                }>
                  {status.remoteDiskSpace ? 
                    formatBytes(status.remoteDiskSpace.free) : 
                    'Not Connected'
                  }
                </Text>
                {status.remoteDiskSpace && (
                  <Text fontSize="xs" color="gray.500">
                    ({status.remoteDiskSpace.percentage.toFixed(1)}% used)
                  </Text>
                )}
              </HStack>
            </VStack>
          </HStack>
        </Flex>

        <StatGroup>
          <Stat size="sm" px={4} py={2} bg={statBg} borderRadius="md">
            <StatLabel color="gray.500">Total Directories</StatLabel>
            <StatNumber fontSize="2xl" color={accentColor}>
              {status.directories.length}
            </StatNumber>
          </Stat>

          <Stat size="sm" px={4} py={2} bg={statBg} borderRadius="md">
            <StatLabel color="gray.500">Completed</StatLabel>
            <StatNumber fontSize="2xl" color="green.500">
              {completedCount}
            </StatNumber>
          </Stat>

          <Stat size="sm" px={4} py={2} bg={statBg} borderRadius="md">
            <StatLabel color="gray.500">Total Size</StatLabel>
            <StatNumber fontSize="2xl">
              {formatBytes(status.totalSize)}
            </StatNumber>
          </Stat>

          <Stat size="sm" px={4} py={2} bg={statBg} borderRadius="md">
            <StatLabel color="gray.500">Speed</StatLabel>
            <StatNumber fontSize="2xl" color={speed > 0 ? accentColor : 'gray.500'}>
              {speed > 0 ? `${(speed / 1048576).toFixed(1)} MB/s` : '—'}
            </StatNumber>
          </Stat>

          {eta && (
            <Stat size="sm" px={4} py={2} bg={statBg} borderRadius="md">
              <StatLabel color="gray.500">ETA</StatLabel>
              <StatNumber fontSize="2xl">
                {eta}
              </StatNumber>
            </Stat>
          )}
        </StatGroup>
      </Flex>
    </Box>
  );
}

function calculateSpeed(status: BackupStatus): number {
  if (!status.startTime || status.completedSize === 0) return 0;
  const elapsed = Date.now() - status.startTime;
  return (status.completedSize / elapsed) * 1000; // bytes per second
}

function calculateETA(status: BackupStatus, bytesPerSecond: number): string | null {
  if (bytesPerSecond === 0) return null;
  const remaining = status.totalSize - status.completedSize;
  const secondsRemaining = remaining / bytesPerSecond;
  return formatDuration(secondsRemaining * 1000);
}