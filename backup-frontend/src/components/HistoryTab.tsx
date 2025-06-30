import { memo } from 'react';
import {
  Box,
  VStack,
  Text,
  Table,
  Thead,
  Tbody,
  Tr,
  Th,
  Td,
  Badge,
  useColorModeValue,
} from '@chakra-ui/react';
import { formatBytes, formatDuration } from '../utils/format';

interface HistoryEntry {
  id: string;
  timestamp: string;
  duration: number;
  size: number;
  filesCount: number;
  status: 'completed' | 'failed' | 'cancelled';
}

interface HistoryTabProps {
  history: HistoryEntry[];
}

export function HistoryTab({ history }: HistoryTabProps) {
  const cardBg = useColorModeValue('white', 'gray.700');
  const borderColor = useColorModeValue('gray.200', 'gray.600');
  const textMuted = useColorModeValue('gray.600', 'gray.400');

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'completed': return 'green';
      case 'failed': return 'red';
      case 'cancelled': return 'yellow';
      default: return 'gray';
    }
  };

  return (
    <Box p={6}>
      <VStack spacing={4} align="stretch">
        <Text fontSize="lg" fontWeight="bold" color="brand.500">
          BACKUP HISTORY
        </Text>
        
        {history.length === 0 ? (
          <Box py={16} textAlign="center">
            <Text color={textMuted}>No backup history available</Text>
          </Box>
        ) : (
          <Box 
            bg={cardBg} 
            borderRadius="md" 
            overflow="hidden"
            borderWidth="1px"
            borderColor={borderColor}
          >
            <Table variant="simple">
              <Thead>
                <Tr>
                  <Th>Date & Time</Th>
                  <Th>Duration</Th>
                  <Th>Size</Th>
                  <Th>Files</Th>
                  <Th>Status</Th>
                </Tr>
              </Thead>
              <Tbody>
                {history.map((entry) => (
                  <Tr key={entry.id}>
                    <Td>{new Date(entry.timestamp).toLocaleString()}</Td>
                    <Td>{formatDuration(entry.duration)}</Td>
                    <Td>{formatBytes(entry.size)}</Td>
                    <Td>{entry.filesCount.toLocaleString()}</Td>
                    <Td>
                      <Badge colorScheme={getStatusColor(entry.status)}>
                        {entry.status.toUpperCase()}
                      </Badge>
                    </Td>
                  </Tr>
                ))}
              </Tbody>
            </Table>
          </Box>
        )}
      </VStack>
    </Box>
  );
}